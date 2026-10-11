// Bounded collection trial composed from live scene candidates, navigation and inventory.
// Parameters: prefab_suffix, item_name (localized), item_key, storage, amount.
// No quest, spending, attack or irreversible dialogue action is performed.
fn resume(memory) {
    // No old key press or camera-dependent calibration is replayed.
    for key in ["waypoint","pending","closing","ui_missing_since"] {memory.remove(key);}
    memory.nav=#{};
    memory
}
fn retire_target(memory) {
    memory.completed_ids.push(memory.target.id);
    memory.retired_positions.push(memory.target.position);
    memory.remove("target");memory.remove("waypoint");memory.remove("pending");memory.remove("looks");memory.nav=#{};
    memory
}
fn tick(ctx) {
    let w=ctx.world;let m=ctx.memory;let g=ctx.goal;
    if w.ui==() {return #{status:"blocked",reason:"Local interface observations required"};}
    if !g.contains("prefab_suffix") || !g.contains("item_name") || !g.contains("item_key") || !g.contains("storage") || !g.contains("amount") {
        return #{status:"blocked",reason:"Explicit resource goal required"};
    }
    if g.amount<1 || g.amount>10 {return #{status:"blocked",reason:"Invalid collection amount"};}
    if ["journal","pause","settings"].contains(w.ui.mode) && !m.contains("target") {
        let closes=if m.contains("closes") {m.closes} else {0};
        if m.contains("closing") && ctx.elapsed_ms-m.closing<4000 {return #{status:"running",memory:m};}
        if closes>=6 {return #{status:"blocked",reason:"Could not leave known menus within retry budget"};}
        m.closes=closes+1;m.closing=ctx.elapsed_ms;
        return #{status:"running",memory:m,action:#{keys:["escape"],ui_mode:w.ui.mode,duration_ms:60}};
    }
    if w.ui.mode!="world" {
        if m.contains("closing") && ctx.elapsed_ms-m.closing<8000 {return #{status:"running",memory:m};}
        if w.ui.mode=="unknown" {
            if !m.contains("ui_missing_since") {m.ui_missing_since=ctx.elapsed_ms;}
            if ctx.elapsed_ms-m.ui_missing_since<15000 {return #{status:"running",memory:m};}
        }
        return #{status:"blocked",reason:"Recognized gameplay required"};
    }
    m.remove("ui_missing_since");
    if !w.player_resolved || w.position==() || w.inventory==() {
        return #{status:"blocked",reason:"Controlled body, position and inventory required"};
    }
    let health=();for stat in w.stats {if stat.kind==0 {health=stat.current;}}
    if health==() || health<=0 {return #{status:"blocked",reason:"Living player health required"};}
    let type_id=item_type_id(g.item_key);
    if type_id==() {return #{status:"blocked",reason:"Unknown or ambiguous resource key"};}
    let current=inventory_count(w.inventory,g.storage,type_id);
    if !m.contains("baseline") {
        m.origin=w.position;m.health=health;m.baseline=current;m.completed_ids=[];m.retired_positions=[];m.selections=0;m.segments=0;m.skipped=0;
    }
    if health<m.health {return #{status:"blocked",reason:"Health decreased during collection trial",memory:m};}
    if current-m.baseline>=g.amount {
        return #{status:"succeeded",reason:"Collection confirmed by inventory increase",memory:#{collected:current-m.baseline,segments:m.segments,targets:m.selections}};
    }
    if distance_xz(w.position.x,w.position.z,m.origin.x,m.origin.z)>85.0 || abs(w.position.y-m.origin.y)>4.0 {
        return #{status:"blocked",reason:"Collection travel or height bound exceeded",memory:m};
    }
    if m.contains("target") && current>m.target_baseline {
        m=retire_target(m);
        return #{status:"running",memory:m};
    }
    if !m.contains("target") {
        if m.selections>=10 {return #{status:"blocked",reason:"Resource selection budget exhausted",memory:m};}
        if w.resource_candidates==() {return #{status:"blocked",reason:"Resource scenes unavailable"};}
        let target=();let nearest=80.0;
        for c in w.resource_candidates {
            if m.completed_ids.contains(c.id) || !c.prefab.ends_with(g.prefab_suffix) || abs(c.position.y-w.position.y)>3.0 {continue;}
            let retired=false;
            for p in m.retired_positions {if distance_xz(c.position.x,c.position.z,p.x,p.z)<0.6 {retired=true;}}
            if retired {continue;}
            let distance=distance_xz(c.position.x,c.position.z,w.position.x,w.position.z);
            if distance<nearest {target=c;nearest=distance;}
        }
        if target==() {return #{status:"blocked",reason:"No matching nearby resource at a suitable altitude"};}
        m.target=target;m.target_baseline=current;m.nav=#{};m.attempts=0;m.selections+=1;
        return #{status:"running",memory:m};
    }
    if m.contains("pending") && ctx.elapsed_ms-m.pending<5000 {return #{status:"running",memory:m};}
    if w.resource_candidates==() {return #{status:"blocked",reason:"Resource observations lost",memory:m};}
    let target=();for c in w.resource_candidates {if c.id==m.target.id && c.prefab==m.target.prefab {target=c;}}
    if target==() {return #{status:"blocked",reason:"Selected resource disappeared or changed identity",memory:m};}
    let distance=distance_xz(target.position.x,target.position.z,w.position.x,w.position.z);
    let label=find_ui_in(w.ui,g.item_name,0.3,0.2,0.8,0.9);
    let pickup=find_ui_in(w.ui,"Prendre",0.3,0.2,0.8,0.9);
    if label!=() && pickup!=() && distance<=2.5 {
        if m.attempts>=3 {return #{status:"blocked",reason:"Pickup attempts did not produce an inventory result",memory:m};}
        m.attempts+=1;m.pending=ctx.elapsed_ms;
        return #{status:"running",memory:m,action:#{keys:["interact"],ui_mode:"world",duration_ms:250}};
    }
    if distance<=1.2 {
        let looks=if m.contains("looks") {m.looks} else {0};
        if looks>=12 {
            m.skipped+=1;m=retire_target(m);
            return #{status:"running",reason:"Unavailable collection scene skipped after bounded inspection",memory:m};
        }
        m.looks=looks+1;
        return #{status:"running",memory:m,action:#{mouse_x:300,mouse_y:if looks==0 {40} else {0},ui_mode:"world",duration_ms:150}};
    }
    let retarget=false;
    if !m.contains("waypoint") {
        if m.segments>=50 {return #{status:"blocked",reason:"Segment budget exhausted",memory:m};}
        let length=min(5.0,distance-0.7);
        m.waypoint=#{x:w.position.x+(target.position.x-w.position.x)*length/distance,z:w.position.z+(target.position.z-w.position.z)*length/distance,radius:0.3,max_distance:8.0,max_height_change:0.75};
        retarget=m.nav.contains("phase");m.segments+=1;
    }
    let result=if retarget {navigation_retarget(w,m.nav,m.waypoint)} else {navigation_step(w,m.nav,m.waypoint)};
    m.nav=result.memory;
    if result.status=="succeeded" {
        m.remove("waypoint");return #{status:"running",memory:m};
    }
    result.memory=m;
    result
}
