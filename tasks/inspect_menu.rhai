// Read-only inspection of three known menu pages. Changes no setting or save.
// Parameter: {"page":"misc" | "settings" | "controls"}.
fn tick(ctx) {
    let ui=ctx.world.ui;let m=ctx.memory;
    if ui==() {return #{status:"blocked",reason:"Interface observations required"};}
    if !ctx.goal.contains("page") {return #{status:"blocked",reason:"Inspection page required"};}
    let route=if ctx.goal.page=="misc" {#{mode:"journal",text:"Divers",top:0.0,bottom:0.15}}
        else if ctx.goal.page=="settings" {#{mode:"pause",text:"Paramètres",top:0.15,bottom:0.95}}
        else if ctx.goal.page=="controls" {#{mode:"settings",text:"Contr",top:0.0,bottom:0.2}}
        else {return #{status:"blocked",reason:"Unsupported inspection page"};};
    if m.contains("selected") {
        if ctx.elapsed_ms-m.selected<5000 {return #{status:"running",memory:m};}
        return #{status:"succeeded",reason:"Requested page captured for inspection",memory:#{mode:ui.mode,lines:ui.lines}};
    }
    if ui.mode!=route.mode {return #{status:"blocked",reason:"Required menu not recognized"};}
    let point=find_ui_in(ui,route.text,0.0,route.top,1.0,route.bottom);
    if point==() {return #{status:"blocked",reason:"Page entry missing or ambiguous"};}
    #{status:"running",memory:#{selected:ctx.elapsed_ms},action:#{cursor:[point.x,point.y],keys:["click"],ui_mode:route.mode,duration_ms:250}}
}
