# ADR 0032: bound composite test workers separately from CLI checks

Deterministic account tests re-execute a test worker with an isolated environment. The shared CLI helper killed that process after eight seconds, even though the worker can perform three turns with separate twenty-second bounds. A full Mac run hit the outer limit before the worker could report its result.

Keep the eight-second limit for short CLI checks. Give composite account workers a ninety-second outer limit that includes all turn bounds, process startup and shutdown. Keep the inner assertions and do not retry failed tests. Timeout diagnostics include both captured streams. The exact source of the original worker delay was not captured; this corrects the inconsistent deadlines rather than claiming a product hang was diagnosed.
