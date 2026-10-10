// Bounded feedback test. It is not a navigation or gathering capability.
fn tick(ctx) {
    let w = ctx.world;
    let m = ctx.memory;
    if !w.player_resolved || w.position == () {
        return #{status: "blocked", reason: "Controlled body and position required"};
    }
    if !m.contains("phase") {
        return #{status:"running",memory:#{phase:1,x:w.position.x,z:w.position.z,started:ctx.elapsed_ms},
                 action:#{keys:["backward"],duration_ms:150}};
    }
    let moved = distance_xz(m.x,m.z,w.position.x,w.position.z);
    if m.phase==1 {
        if ctx.elapsed_ms-m.started < 600 {
            return #{status:"running",memory:m};
        }
        if moved < 0.02 {
            return #{status:"blocked",reason:"No measured movement after normal input"};
        }
        return #{status:"running",memory:#{phase:2,x:m.x,z:m.z,moved:moved,started:ctx.elapsed_ms},
                 action:#{keys:["forward"],duration_ms:150}};
    }
    if ctx.elapsed_ms-m.started < 600 {
        return #{status:"running",memory:m};
    }
    #{status:"succeeded",reason:"Movement measured; inputs released",memory:#{forward_distance:m.moved,return_distance:moved}}
}
