# goose-router

Native, in-process cost-saving model router for goose.

A **router** scores the current conversation and decides which model a turn
should be served by. This is the engine behind goose's **cost savings mode**:
simple turns go to a small/cheap model, hard turns stay on the frontier model.

Everything runs in-process and fully offline from a local bundle — there is no
sidecar, proxy, or network dependency at routing time.

```rust
pub trait Router: Send + Sync {
    fn name(&self) -> &'static str;
    fn route(&self, conversation: &Conversation) -> Option<RouteDecision>;
}
```

`route` returns a `RouteDecision` carrying the estimated `complexity` and the
`selected_model` (the ladder rung to use, or `None` to keep the main model).

## How it works

The `EmbeddingRouter`:

1. Renders the recent conversation to a short text view, anchoring the latest
   *real* user turn with `>>>`. goose injects tool-narration as user-role
   messages flagged `user_visible: false`; because we run natively we read that
   flag directly and anchor on real intent (an HTTP proxy loses the flag and
   has to guess with regex).
2. Embeds it with a small ONNX encoder (mean-pooled), CPU, a few milliseconds.
3. Runs a tiny MLP head to produce a complexity score in `[0, 1]`.
4. Maps the score to a model on a **ladder** using band thresholds.

It loads a bundle from `~/.goose/complexity_model/`:

```
config.json            # bundle metadata (embedder + head + routing bands)
embedder.onnx          # ONNX encoder
tokenizer.json         # HF tokenizer
weights.safetensors    # MLP head weights
```

If the bundle is absent, the router is simply disabled (no error).

## The ladder

A ladder is an ordered list of models, cheapest first, plus ascending band
thresholds that map a complexity score to a rung. For `[fast, mid, frontier]`
with bands `[0.40, 0.65]`:

```
complexity < 0.40         -> fast
0.40 <= complexity < 0.65 -> mid
complexity >= 0.65        -> frontier
```

The **model names** come from you (`GOOSE_ROUTER_LADDER`). The **bands** are a
property of the trained bundle — its `config.json` carries fitted defaults
under `routing.complexity_bands_default`, so you don't have to invent
thresholds. `GOOSE_ROUTER_BANDS` overrides them if you want to retune. With no
usable bands, evenly spaced thresholds are used.

The selected model name is resolved against your current provider, so it picks
up context limits, temperature, and other provider defaults.

## Enabling in goose

Cost savings mode is off by default.

```bash
export GOOSE_COST_SAVINGS_MODE=true
export GOOSE_ROUTER_LADDER="gpt-4o-mini,gpt-4o,o3"   # cheap -> dear
# optional: override the bundle's fitted bands
# export GOOSE_ROUTER_BANDS="0.40,0.65"
```

Then drop a bundle into `~/.goose/complexity_model/`. A single-model ladder
(`GOOSE_ROUTER_LADDER="gpt-4o-mini"`) acts as a simple "use the cheap model
below the first band" switch.

Everything is fail-open: a missing bundle, scoring error, or unresolvable model
all fall back to the main session model.

## Environment variables

| Variable | Purpose |
|---|---|
| `GOOSE_COST_SAVINGS_MODE` | Master on/off switch (default `false`) |
| `GOOSE_ROUTER_LADDER` | Comma-separated model names, cheap → dear. No ladder ⇒ routing stays on the main model. |
| `GOOSE_ROUTER_BANDS` | Ascending complexity thresholds overriding the bundle's fitted defaults |
