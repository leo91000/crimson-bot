// Inspects the quest UI through the runtime's local OCR. No quest is accepted.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() {return #{status:"blocked",reason:"Local screen perception required"};}
    if ui.mode=="journal" {
        return #{status:"succeeded",reason:"Journal text captured for quest selection",memory:#{mode:ui.mode,lines:ui.lines}};
    }
    if !ctx.memory.contains("opened") {
        if ui.mode!="world" {return #{status:"blocked",reason:"Expected gameplay before opening journal"};}
        return #{status:"running",memory:#{opened:true,started:ctx.elapsed_ms},action:#{keys:["journal"],duration_ms:250}};
    }
    if ctx.elapsed_ms-ctx.memory.started < 12000 {
        return #{status:"running",memory:ctx.memory};
    }
    #{status:"blocked",reason:"Journal did not become recognizable within deadline",memory:#{mode:ui.mode,lines:ui.lines}}
}
