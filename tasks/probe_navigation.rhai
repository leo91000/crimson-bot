// Small on-machine navigation trial: measure a safe short retreat, then target
// one unit along that observed direction. No interaction or combat is performed.
fn tick(ctx) {
    let w=ctx.world;let m=ctx.memory;
    if w.ui==() || w.ui.mode!="world" || !w.player_resolved || w.position==() {
        return #{status:"blocked",reason:"Gameplay observations required for navigation trial"};
    }
    if !m.contains("anchor") {
        return #{status:"running",memory:#{anchor:w.position,started:ctx.elapsed_ms},action:#{keys:["backward"],ui_mode:"world",duration_ms:150}};
    }
    if !m.contains("goal") {
        if ctx.elapsed_ms-m.started<800 {return #{status:"running",memory:m};}
        let dx=w.position.x-m.anchor.x;let dz=w.position.z-m.anchor.z;
        let length=distance_xz(w.position.x,w.position.z,m.anchor.x,m.anchor.z);
        if length<0.02 || abs(w.position.y-m.anchor.y)>0.5 {
            return #{status:"blocked",reason:"Retreat did not establish a safe local direction"};
        }
        return #{status:"running",memory:#{anchor:m.anchor,goal:#{x:m.anchor.x+dx/length,z:m.anchor.z+dz/length,radius:0.2,max_distance:2.0,max_height_change:0.5},nav:#{}}};
    }
    let decision=navigation_step(w,m.nav,m.goal);
    decision.memory=#{anchor:m.anchor,goal:m.goal,nav:decision.memory};
    decision
}
