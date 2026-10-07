//! Owns unchanged raw agy argv and inherited terminal or bounded-tail tee output.
use crate::AgyRunner;
use crate::Decoder;
use antiquota_core::{PortError, TurnRequest, TurnResult};
use process_wrap::tokio::{ChildWrapper, CommandWrap, KillOnDrop};
use std::{
    io::{IsTerminal, Write},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt, process::Command};
pub struct RawResult {
    pub code: i32,
    pub out: String,
    pub err: String,
    pub decoded: Option<TurnResult>,
}
struct Pump {
    tail: Vec<u8>,
    decoder: Option<Decoder>,
}
struct RawChild(Box<dyn ChildWrapper>);
impl Drop for RawChild {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}
async fn tee(mut source: impl tokio::io::AsyncRead + Unpin, out: bool) -> Result<Pump, PortError> {
    let mut tail = vec![];
    let mut chunk = [0; 65536];
    let mut line = vec![];
    let mut decoder = Decoder::new(0);
    let mut seen = false;
    let mut oversized = false;
    loop {
        let n = source
            .read(&mut chunk)
            .await
            .map_err(|e| PortError::Unavailable(e.to_string()))?;
        if n == 0 {
            break;
        }
        let result = if out {
            let mut writer = std::io::stdout().lock();
            writer.write_all(&chunk[..n]).and_then(|_| writer.flush())
        } else {
            let mut writer = std::io::stderr().lock();
            writer.write_all(&chunk[..n]).and_then(|_| writer.flush())
        };
        result.map_err(|e| PortError::Unavailable(e.to_string()))?;
        if out {
            for byte in &chunk[..n] {
                if *byte == b'\n' {
                    if !oversized
                        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line)
                        && value["event"].is_string()
                    {
                        seen = true;
                        let _ = decoder.accept(value);
                    }
                    line.clear();
                    oversized = false;
                } else if line.len() < 4_000_000 {
                    line.push(*byte);
                } else {
                    oversized = true;
                }
            }
        }
        tail.extend_from_slice(&chunk[..n]);
        if tail.len() > 65536 {
            tail.drain(..tail.len() - 65536);
        }
    }
    Ok(Pump {
        tail,
        decoder: seen.then_some(decoder),
    })
}
impl AgyRunner {
    pub async fn raw(
        &self,
        request: &TurnRequest,
        args: &[String],
    ) -> Result<RawResult, PortError> {
        self.ensure_executable()?;
        let interactive = std::io::stdout().is_terminal();
        if !interactive
            && args
                .iter()
                .any(|a| a == "-p" || a == "--prompt" || a.starts_with("--prompt="))
        {
            self.preflight(request).await?;
        }
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .current_dir(&request.cwd)
            .env("HOME", &request.slot.home)
            .env("USERPROFILE", &request.slot.home)
            .env("SSH_CONNECTION", "127.0.0.1 0 127.0.0.1 0")
            .env("ANTIQUOTA_SLOT", &request.slot.name)
            .stdin(Stdio::inherit())
            .kill_on_drop(true);
        if interactive {
            // Register a Rust signal handler before exec; agy inherits native TTY signal delivery.
            // https://docs.rs/tokio/1.53.2/tokio/signal/fn.ctrl_c.html
            let listener = tokio::spawn(async {
                let _ = tokio::signal::ctrl_c().await;
            });
            tokio::task::yield_now().await;
            let result = command
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await
                .map_err(|e| PortError::Unavailable(e.to_string()));
            listener.abort();
            return Ok(RawResult {
                code: result?.code().unwrap_or(1),
                out: String::new(),
                err: String::new(),
                decoded: None,
            });
        }
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(KillOnDrop);
        #[cfg(unix)]
        wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(process_wrap::tokio::JobObject);
        let mut child = RawChild(
            wrapped
                .spawn()
                .map_err(|e| PortError::Unavailable(e.to_string()))?,
        );
        let stdout = child
            .0
            .stdout()
            .take()
            .ok_or_else(|| PortError::Unavailable("raw stdout unavailable".into()))?;
        let stderr = child
            .0
            .stderr()
            .take()
            .ok_or_else(|| PortError::Unavailable("raw stderr unavailable".into()))?;
        let mut output = tokio::spawn(tee(stdout, true));
        let mut error = tokio::spawn(tee(stderr, false));
        let mut out = None;
        let mut err = None;
        let mut interrupted = false;
        let signal = tokio::signal::ctrl_c();
        tokio::pin!(signal);
        let outcome:Result<_,PortError>=async {let status=loop {
            if output.is_finished() && out.is_none(){out=Some((&mut output).await.map_err(|e|PortError::Unavailable(e.to_string()))??);}
            if error.is_finished() && err.is_none(){err=Some((&mut error).await.map_err(|e|PortError::Unavailable(e.to_string()))??);}
            if let Some(status)=child.0.try_wait().map_err(|e|PortError::Unavailable(e.to_string()))?{break status;}
            tokio::select!{_=&mut signal, if !interrupted=>{interrupted=true;let _=child.0.start_kill();},_=tokio::time::sleep(Duration::from_millis(10))=>{}}
        };let _=child.0.start_kill();
        let out=match out{Some(out)=>out,None=>tokio::time::timeout(Duration::from_secs(1),&mut output).await.map_err(|_|PortError::Unavailable("raw stdout did not close".into()))?.map_err(|e|PortError::Unavailable(e.to_string()))??};
        let err=match err{Some(err)=>err,None=>tokio::time::timeout(Duration::from_secs(1),&mut error).await.map_err(|_|PortError::Unavailable("raw stderr did not close".into()))?.map_err(|e|PortError::Unavailable(e.to_string()))??};
        let code=status.code().unwrap_or(1);let err=String::from_utf8_lossy(&err.tail).into_owned();let decoded=out.decoder.map(|decoder|decoder.finish(code,false,&err));
        Ok(RawResult{code,out:String::from_utf8_lossy(&out.tail).into_owned(),err,decoded})}.await;
        output.abort();
        error.abort();
        outcome
    }
}
