# ADR 0030: finish contract responses at their HTTP frame

The loopback contract client read every response until socket EOF. On macOS, a GET body sent after the request headers can reach the server after its response is complete. Closing that connection can produce a reset after all declared response bytes have arrived. Waiting for EOF then rejects a complete response and makes unrelated recovery tests flaky.

Read exactly Content-Length bytes, or read chunks through the zero chunk and complete trailers. Responses without framing still require a clean EOF. HEAD and status codes without a body return an empty body. A reset or EOF before the declared frame is complete remains a failure. Do not retry a request or accept a truncated response.

Pure reader regressions supply a reset immediately after a complete frame and reject incomplete lengths, chunks and trailers. Existing HTTP, captured-route and abrupt-engine recovery tests exercise the same shared helper. Production transport and captured fixtures remain unchanged.
