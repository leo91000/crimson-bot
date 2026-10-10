// Goal parameters: x, z, optional radius, max_distance, max_height_change.
// Local waypoint steering, with calibration and observed progress. No world routing.
fn tick(ctx) {
    navigation_step(ctx.world,ctx.memory,ctx.goal)
}
