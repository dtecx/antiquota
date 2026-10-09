# ADR 0028: Fake ports and clock-driven schedules

Status: Accepted for Slice 16b.

Every port app uses for real work now has a fake in app tests: the runner, accounts, store, quota probe, version control and terminals. The fakes record what app asks of them, so a test can check the order of calls and the state app keeps, without git, a shell or agy.

Loops and daily schedules already read the time only through the `Clock` port in `scheduler_tick`. The scheduler loop still wakes on a real timer, but whether a loop is due and when it fires next come from the clock. Tests therefore move a fake clock and call `scheduler_tick` instead of waiting. The whole-engine version uses the real store, accounts and agy adapters with fake-agy.

Properties use proptest. Account ranking is checked against rules stated in the test rather than against a copy of the ranking code. The event-log property cuts the last write inside its JSON object, so a torn line can never read back as complete.
