// Select an existing journal entry and verify its detail title before reporting.
// Parameters: {"quest":"unique word in the desired title","expand":false,"max_scrolls":0}.
fn tick(ctx) {
    let ui=ctx.world.ui;
    if ui==() || ui.mode!="journal" {return #{status:"blocked",reason:"Journal required"};}
    if !ctx.goal.contains("quest") {return #{status:"blocked",reason:"Quest selector required"};}
    if !ctx.memory.contains("selected") {
        let point=find_ui_in(ui,ctx.goal.quest,0.0,0.15,0.45,0.9);
        if point==() {
            let m=ctx.memory;
            let limit=if ctx.goal.contains("max_scrolls") {ctx.goal.max_scrolls} else {0};
            if limit<0 || limit>16 {return #{status:"blocked",reason:"Scroll budget outside 0..16"};}
            if m.contains("scroll_started") && ctx.elapsed_ms-m.scroll_started<3000 {return #{status:"running",memory:m};}
            let scrolls=if m.contains("scrolls") {m.scrolls} else {0};
            if scrolls>=limit {return #{status:"blocked",reason:"Quest title absent or ambiguous within scroll budget",memory:m};}
            let visible=[];
            for line in ui.lines {if line.left<ui.width*0.45 && line.top>ui.height*0.15 && line.top<ui.height*0.9 {visible.push(line.text);}}
            let repeats=if m.contains("visible") && m.visible==visible {if m.contains("repeats") {m.repeats+1} else {1}} else {0};
            if repeats>=2 {return #{status:"blocked",reason:"Quest list did not change after scrolling",memory:m};}
            m.visible=visible;m.repeats=repeats;m.scrolls=scrolls+1;m.scroll_started=ctx.elapsed_ms;
            return #{status:"running",memory:m,action:#{cursor:[(ui.width*0.22).to_int(),(ui.height*0.6).to_int()],wheel_y:-3,ui_mode:"journal",duration_ms:100}};
        }
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
