// Validate all four configured movement bindings before navigating.
// Stops on missing movement, height change, lost interface, health or inventory change.
fn tick(ctx) {
    let w=ctx.world;let m=ctx.memory;let names=["forward","backward","left","right"];
    if w.ui==() {return #{status:"blocked",reason:"Interface observations required"};}
    if !m.contains("index") && ["journal","pause","settings"].contains(w.ui.mode) {
        if m.contains("closing") && ctx.elapsed_ms-m.closing<4000 {return #{status:"running",memory:m};}
        let closes=if m.contains("closes") {m.closes} else {0};
        if closes>=6 {return #{status:"blocked",reason:"Could not leave known menus"};}
        return #{status:"running",memory:#{closes:closes+1,closing:ctx.elapsed_ms},action:#{keys:["escape"],ui_mode:w.ui.mode,duration_ms:250}};
    }
    if w.ui.mode!="world" {
        if m.contains("closing") && ctx.elapsed_ms-m.closing<10000 {return #{status:"running",memory:m};}
        if w.ui.mode=="unknown" {
            if !m.contains("missing_since") {m.missing_since=ctx.elapsed_ms;}
            if ctx.elapsed_ms-m.missing_since<15000 {return #{status:"running",memory:m};}
        }
        return #{status:"blocked",reason:"Known gameplay required for binding validation"};
    }
    m.remove("missing_since");
    if !w.player_resolved || w.position==() || w.inventory==() {return #{status:"blocked",reason:"Body, position and inventory required"};}
    let health=();for s in w.stats {if s.kind==0 {health=s.current;}}
    if health==() || health<=0 {return #{status:"blocked",reason:"Living health required"};}
    if !m.contains("index") {
        return #{status:"running",memory:#{index:0,previous:w.position,origin:w.position,health:health,inventory:w.inventory,steps:[],started:ctx.elapsed_ms},action:#{keys:[names[0]],ui_mode:"world",duration_ms:150}};
    }
    if health!=m.health || w.inventory!=m.inventory {return #{status:"blocked",reason:"Health or inventory changed during binding validation"};}
    if ctx.elapsed_ms-m.started<800 {return #{status:"running",memory:m};}
    let dx=w.position.x-m.previous.x;let dz=w.position.z-m.previous.z;
    let length=distance_xz(w.position.x,w.position.z,m.previous.x,m.previous.z);
    if length<0.02 || length>2.0 || abs(w.position.y-m.origin.y)>0.5 {
        return #{status:"blocked",reason:"Configured movement did not produce a bounded displacement",memory:#{binding:names[m.index],distance:length}};
    }
    m.steps.push(#{binding:names[m.index],x:dx/length,z:dz/length,distance:length});
    m.index+=1;
    if m.index>=4 {
        let s=m.steps;
        if s[0].x*s[1].x+s[0].z*s[1].z> -0.7 || s[2].x*s[3].x+s[2].z*s[3].z> -0.7 || abs(s[0].x*s[3].x+s[0].z*s[3].z)>0.75 {
            return #{status:"blocked",reason:"Movement bindings are not opposing independent axes",memory:#{steps:s}};
        }
        return #{status:"succeeded",reason:"All four movement bindings verified",memory:#{steps:s}};
    }
    m.previous=w.position;m.started=ctx.elapsed_ms;
    #{status:"running",memory:m,action:#{keys:[names[m.index]],ui_mode:"world",duration_ms:150}}
}
