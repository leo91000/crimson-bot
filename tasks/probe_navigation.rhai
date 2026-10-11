// Small on-machine navigation trial: measure a safe short retreat, then target
// one unit along that observed direction. No interaction or combat is performed.
fn tick(ctx) {
    let w=ctx.world;let m=ctx.memory;let g=ctx.goal;
    if w.ui!=() && ["journal","pause","settings"].contains(w.ui.mode) && !m.contains("anchor") {
        if m.contains("closing") && ctx.elapsed_ms-m.closing<4000 {return #{status:"running",memory:m};}
        let closes=if m.contains("closes") {m.closes} else {0};
        if closes>=6 {return #{status:"blocked",reason:"Known menus did not close within budget"};}
        m.closes=closes+1;m.closing=ctx.elapsed_ms;
        return #{status:"running",memory:m,action:#{keys:["escape"],ui_mode:w.ui.mode,duration_ms:60}};
    }
    if w.ui!=() && w.ui.mode=="unknown" && m.contains("closing") && ctx.elapsed_ms-m.closing<8000 {
        return #{status:"running",memory:m};
    }
    if w.ui==() || w.ui.mode!="world" || !w.player_resolved || w.position==() {
        return #{status:"blocked",reason:"Gameplay observations required for navigation trial"};
    }
    let health=();for stat in w.stats {if stat.kind==0 {health=stat.current;}}
    if health==() || health<=0 || w.inventory==() {return #{status:"blocked",reason:"Living player and known inventory required"};}
    if m.contains("health") && (health<m.health || w.inventory!=m.inventory) {
        return #{status:"blocked",reason:"Health or inventory changed during navigation trial",memory:m};
    }
    if !m.contains("anchor") {
        m.anchor=w.position;m.started=ctx.elapsed_ms;m.health=health;m.inventory=w.inventory;
        return #{status:"running",memory:m,action:#{keys:["backward"],ui_mode:"world",duration_ms:150}};
    }
    if !m.contains("goal") {
        if ctx.elapsed_ms-m.started<800 {return #{status:"running",memory:m};}
        let dx=w.position.x-m.anchor.x;let dz=w.position.z-m.anchor.z;
        let length=distance_xz(w.position.x,w.position.z,m.anchor.x,m.anchor.z);
        if length<0.02 || abs(w.position.y-m.anchor.y)>0.5 {
            return #{status:"blocked",reason:"Retreat did not establish a safe local direction"};
        }
        let distance=if g.contains("distance") {g.distance} else {1.0};
        let lateral=if g.contains("lateral") {g.lateral} else {0.0};
        if distance<0.5 || distance>3.0 || abs(lateral)>1.0 {
            return #{status:"blocked",reason:"Invalid bounded navigation trial goal"};
        }
        m.goal=#{x:m.anchor.x+(dx*distance-dz*lateral)/length,z:m.anchor.z+(dz*distance+dx*lateral)/length,radius:0.2,max_distance:5.0,max_height_change:0.5};m.nav=#{};
        return #{status:"running",memory:m};
    }
    let decision=navigation_step(w,m.nav,m.goal);
    m.nav=decision.memory;decision.memory=m;
    decision
}
