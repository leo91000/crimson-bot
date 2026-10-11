// Parameterized read-only selection in the faction journal.
// Parameters: {"group":"visible unique word","region":[left,top,right,bottom]} (region optional).
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() || ui.mode!="journal" {return #{status:"blocked",reason:"Recognized journal required"};}
    if !ctx.goal.contains("group") {return #{status:"blocked",reason:"Quest group parameter required"};}
    if !ctx.memory.contains("factions_started") {
        let tab=find_ui_in(ui,"Factions",0.0,0.0,1.0,0.15);
        if tab==() {return #{status:"blocked",reason:"Faction tab header missing or ambiguous"};}
        return #{status:"running",memory:#{factions_started:ctx.elapsed_ms},action:#{cursor:[tab.x,tab.y],keys:["click"],ui_mode:"journal",duration_ms:250}};
    }
    if ctx.elapsed_ms-ctx.memory.factions_started<6000 {return #{status:"running",memory:ctx.memory};}
    if !ctx.memory.contains("factions_ready") {
        if ui.page=="faction_index" {
            let memory=ctx.memory;memory.factions_ready=true;
            return #{status:"running",memory:memory};
        }
        if ctx.elapsed_ms-ctx.memory.factions_started<15000 {return #{status:"running",memory:ctx.memory};}
        return #{status:"blocked",reason:"Faction index was not verified after tab selection"};
    }
    if !ctx.memory.contains("selected") {
        if ctx.memory.contains("return_started") && ctx.elapsed_ms-ctx.memory.return_started<5000 {
            return #{status:"running",memory:ctx.memory};
        }
        let region=if ctx.goal.contains("region") {ctx.goal.region} else {[0.0,0.15,1.0,0.95]};
        if region.len()!=4 {return #{status:"blocked",reason:"Four normalized selector bounds required"};}
        let point=find_ui_in(ui,ctx.goal.group,region[0].to_float(),region[1].to_float(),region[2].to_float(),region[3].to_float());
        if point==() {
            let returns=if ctx.memory.contains("returns") {ctx.memory.returns} else {0};
            if returns>=2 {return #{status:"blocked",reason:"Quest group missing or ambiguous after returning to parent"};}
            return #{status:"running",memory:#{factions_started:ctx.memory.factions_started,factions_ready:true,returns:returns+1,return_started:ctx.elapsed_ms},action:#{keys:["escape"],ui_mode:"journal",duration_ms:60}};
        }
        return #{status:"running",memory:#{factions_started:ctx.memory.factions_started,factions_ready:true,selected:true,started:ctx.elapsed_ms},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:"journal",duration_ms:250}};
    }
    if ctx.elapsed_ms-ctx.memory.started<7000 {return #{status:"running",memory:ctx.memory};}
    let title=find_ui_in(ui,ctx.goal.group,0.0,0.1,0.3,0.35);
    if title!=() {return #{status:"succeeded",reason:"Faction quest group title verified",memory:#{lines:ui.lines}};}
    if ctx.elapsed_ms-ctx.memory.started<18000 {return #{status:"running",memory:ctx.memory};}
    #{status:"blocked",reason:"Quest group title not verified after selection",memory:#{lines:ui.lines}}
}
