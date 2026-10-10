// Read the secondary-quest tab. Selection only; never accepts or validates quests.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() || ui.mode!="journal" {return #{status:"blocked",reason:"Recognized journal required"};}
    if !ctx.memory.contains("selected") {
        let point=find_ui(ui,"Factions");
        if point==() {return #{status:"blocked",reason:"Faction tab is missing or ambiguous"};}
        return #{status:"running",memory:#{selected:true,started:ctx.elapsed_ms},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:"journal",duration_ms:150}};
    }
    if ctx.elapsed_ms-ctx.memory.started<2000 {return #{status:"running",memory:ctx.memory};}
    #{status:"succeeded",reason:"Journal captured after tab selection; content must be checked",memory:#{lines:ui.lines}}
}
