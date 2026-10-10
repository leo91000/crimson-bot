// Explicit --allow-title is required by the runtime. A verified backup precedes input.
// Selects Continue only; never New Game, an arbitrary save, or cloud conflict choices.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() {return #{status:"blocked",reason:"Screen perception required"};}
    if ui.mode=="world" && ctx.world.player_resolved {return #{status:"succeeded",reason:"Gameplay ready"};}
    if ctx.elapsed_ms>90000 {return #{status:"blocked",reason:"Gameplay did not become ready within deadline"};}
    if ui.mode=="title" && !ctx.memory.contains("continued") {
        let point=find_ui(ui,"Continuer");
        if point==() {point=find_ui(ui,"Reprendre");}
        if point==() {point=find_ui_in(ui,"Jouer",0.0,0.7,1.0,1.0);}
        if point==() {return #{status:"blocked",reason:"Continue option not uniquely identified"};}
        return #{status:"running",memory:#{continued:true},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:"title",duration_ms:250}};
    }
    #{status:"running",memory:ctx.memory}
}
