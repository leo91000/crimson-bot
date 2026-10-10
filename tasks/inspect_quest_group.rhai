// Parameterized read-only selection in the faction journal.
// Parameters: {"group":"visible unique word"}.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() || ui.mode!="journal" {return #{status:"blocked",reason:"Recognized journal required"};}
    if !ctx.goal.contains("group") {return #{status:"blocked",reason:"Quest group parameter required"};}
    if !ctx.memory.contains("selected") {
        let point=find_ui(ui,ctx.goal.group);
        if point==() {return #{status:"blocked",reason:"Quest group missing or ambiguous"};}
        return #{status:"running",memory:#{selected:true,started:ctx.elapsed_ms},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:"journal",duration_ms:250}};
    }
    if ctx.elapsed_ms-ctx.memory.started<7000 {return #{status:"running",memory:ctx.memory};}
    #{status:"succeeded",reason:"Quest group observation captured",memory:#{lines:ui.lines}}
}
