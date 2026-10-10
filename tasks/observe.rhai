// Passive task: records world observations, then completes without controls.
fn tick(ctx) {
    if ctx.elapsed_ms >= 2000 {
        return #{status: "succeeded", reason: "Observation period complete"};
    }
    #{status: "running"}
}
