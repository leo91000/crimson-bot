// Leaf capability: collect an identified resource already within interaction range.
// It does not navigate or choose an item. Parameters: item_name, stable item_key,
// storage (typically 1), amount. Completion requires an observed inventory gain.
fn tick(ctx) {
    let w=ctx.world;let goal=ctx.goal;let m=ctx.memory;
    if !goal.contains("item_name") || !goal.contains("item_key") || !goal.contains("storage") || !goal.contains("amount") {
        return #{status:"blocked",reason:"Explicit resource and inventory parameters required"};
    }
    if goal.amount<1 || goal.amount>10 {return #{status:"blocked",reason:"Gather amount outside bounded trial"};}
    if w.ui==() || w.ui.mode!="world" || w.inventory==() {return #{status:"blocked",reason:"Gameplay and current inventory required"};}
    let type_id=item_type_id(goal.item_key);
    if type_id==() {return #{status:"blocked",reason:"Resource key not uniquely resolved in live catalog"};}
    let current=inventory_count(w.inventory,goal.storage,type_id);
    if !m.contains("baseline") {
        return #{status:"running",memory:#{baseline:current,attempts:0}};
    }
    if current-m.baseline>=goal.amount {
        return #{status:"succeeded",reason:"Resource increase confirmed in inventory",memory:#{collected:current-m.baseline}};
    }
    if m.contains("pending") && ctx.elapsed_ms-m.pending<5000 {return #{status:"running",memory:m};}
    if m.attempts>=3 {return #{status:"blocked",reason:"No confirmed inventory result within retry budget"};}
    let label=find_ui_in(w.ui,goal.item_name,0.3,0.2,0.8,0.9);
    let pickup=find_ui_in(w.ui,"Prendre",0.3,0.2,0.8,0.9);
    if label==() || pickup==() {return #{status:"blocked",reason:"Named resource and pickup prompt not verified"};}
    #{status:"running",memory:#{baseline:m.baseline,attempts:m.attempts+1,pending:ctx.elapsed_ms},
      action:#{keys:["interact"],ui_mode:"world",duration_ms:250}}
}
