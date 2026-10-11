// Select an existing journal entry and verify its detail title before reporting.
// Parameters: {"quest":"unique word in the desired title","expand":false}.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() || ui.mode!="journal" {return #{status:"blocked",reason:"Journal required"};}
    if !ctx.goal.contains("quest") {return #{status:"blocked",reason:"Quest selector required"};}
    if !ctx.memory.contains("selected") {
        let point=find_ui_in(ui,ctx.goal.quest,0.0,0.15,0.45,0.9);
        if point==() {return #{status:"blocked",reason:"Quest title absent or ambiguous in list"};}
        return #{status:"running",memory:#{selected:true,started:ctx.elapsed_ms},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:"journal",duration_ms:250}};
    }
    if ctx.elapsed_ms-ctx.memory.started<7000 {return #{status:"running",memory:ctx.memory};}
    let title=find_ui_in(ui,ctx.goal.quest,0.45,0.12,1.0,0.3);
    if title!=() && ctx.goal.contains("expand") && ctx.goal.expand && !ctx.memory.contains("expanded") {
        let memory=ctx.memory;memory.expanded=true;memory.started=ctx.elapsed_ms;
        return #{status:"running",memory:memory,action:#{keys:["expand"],ui_mode:"journal",duration_ms:250}};
    }
    if title!=() {return #{status:"succeeded",reason:"Quest detail title verified",memory:#{lines:ui.lines}};}
    if ctx.elapsed_ms-ctx.memory.started<18000 {return #{status:"running",memory:ctx.memory};}
    #{status:"blocked",reason:"Quest detail title was not verified",memory:#{lines:ui.lines}}
}
