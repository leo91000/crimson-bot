// Bounded collection trial composed from live scene candidates, navigation and inventory.
// Parameters: prefab_suffix, item_name (localized), item_key, storage, amount.
// No quest, spending, attack or irreversible dialogue action is performed.
fn tick(ctx) {
    let w=ctx.world;let m=ctx.memory;let g=ctx.goal;
    if w.ui==() {return #{status:"blocked",reason:"Local interface observations required"};}
    if !g.contains("prefab_suffix") || !g.contains("item_name") || !g.contains("item_key") || !g.contains("storage") || !g.contains("amount") {
        return #{status:"blocked",reason:"Explicit resource goal required"};
    }
    if g.amount<1 || g.amount>10 {return #{status:"blocked",reason:"Invalid collection amount"};}
    if w.ui.mode=="journal" && !m.contains("target") {
        let closes=if m.contains("closes") {m.closes} else {0};
        if m.contains("closing") && ctx.elapsed_ms-m.closing<4000 {return #{status:"running",memory:m};}
        if closes>=3 {return #{status:"blocked",reason:"Could not leave journal within retry budget"};}
        return #{status:"running",memory:#{closes:closes+1,closing:ctx.elapsed_ms},action:#{keys:["escape"],ui_mode:"journal",duration_ms:250}};
    }
    if w.ui.mode!="world" {
        if m.contains("closing") && ctx.elapsed_ms-m.closing<8000 {return #{status:"running",memory:m};}
        return #{status:"blocked",reason:"Recognized gameplay required"};
    }
    if !w.player_resolved || w.position==() || w.inventory==() {
        return #{status:"blocked",reason:"Controlled body, position and inventory required"};
    }
    let health=();for stat in w.stats {if stat.kind==0 {health=stat.current;}}
    if health==() || health<=0 {return #{status:"blocked",reason:"Living player health required"};}
    let type_id=item_type_id(g.item_key);
    if type_id==() {return #{status:"blocked",reason:"Unknown or ambiguous resource key"};}
    let current=inventory_count(w.inventory,g.storage,type_id);
    if !m.contains("target") {
        if w.resource_candidates==() {return #{status:"blocked",reason:"Resource scenes unavailable"};}
        let target=();let nearest=80.0;
        for c in w.resource_candidates {
            if !c.prefab.ends_with(g.prefab_suffix) || abs(c.position.y-w.position.y)>3.0 {continue;}
            let distance=distance_xz(c.position.x,c.position.z,w.position.x,w.position.z);
            if distance<nearest {target=c;nearest=distance;}
        }
        if target==() {return #{status:"blocked",reason:"No matching nearby resource at a suitable altitude"};}
        return #{status:"running",memory:#{target:target,origin:w.position,health:health,baseline:current,nav:#{},attempts:0,segments:0}};
    }
    if health<m.health {return #{status:"blocked",reason:"Health decreased during collection trial",memory:m};}
    if current-m.baseline>=g.amount {
        return #{status:"succeeded",reason:"Collection confirmed by inventory increase",memory:#{collected:current-m.baseline,segments:m.segments}};
    }
    if distance_xz(w.position.x,w.position.z,m.origin.x,m.origin.z)>85.0 || abs(w.position.y-m.origin.y)>4.0 {
        return #{status:"blocked",reason:"Collection travel or height bound exceeded",memory:m};
    }
    if m.contains("pending") && ctx.elapsed_ms-m.pending<5000 {return #{status:"running",memory:m};}
    if w.resource_candidates==() {return #{status:"blocked",reason:"Resource observations lost",memory:m};}
    let target=();for c in w.resource_candidates {if c.id==m.target.id && c.prefab==m.target.prefab {target=c;}}
    if target==() {return #{status:"blocked",reason:"Selected resource disappeared or changed identity",memory:m};}
    let label=find_ui_in(w.ui,g.item_name,0.3,0.2,0.8,0.9);
    let pickup=find_ui_in(w.ui,"Prendre",0.3,0.2,0.8,0.9);
    if label!=() && pickup!=() {
        if m.attempts>=3 {return #{status:"blocked",reason:"Pickup attempts did not produce an inventory result",memory:m};}
        m.attempts+=1;m.pending=ctx.elapsed_ms;
        return #{status:"running",memory:m,action:#{keys:["interact"],ui_mode:"world",duration_ms:250}};
    }
    let distance=distance_xz(target.position.x,target.position.z,w.position.x,w.position.z);
    if distance<=1.0 {return #{status:"blocked",reason:"Near resource but named interaction prompt is not verified",memory:m};}
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
