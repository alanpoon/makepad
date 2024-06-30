use {
    crate::{
        makepad_platform::*,
        turtle::{Walk, Size, Align},
        font_atlas::{CxFontsAtlasTodo, CxFont, CxFontAtlas, Font, GlyphInfo},
        draw_list_2d::ManyInstances,
        geometry::GeometryQuad2D,
        cx_2d::Cx2d
    },
    makepad_rustybuzz::Direction,
};

const LOGICAL_PIXELS_PER_INCH: f64 = 96.0;
const POINTS_PER_INCH: f64 = 72.0;
const ZBIAS_STEP: f32 = 0.00001;

live_design!{
    
    DrawText = {{DrawText}} {
        //debug: true;
        color: #fff
        
        uniform brightness: float
        uniform curve: float
        uniform sdf_radius: float
        uniform sdf_cutoff: float
        
        texture tex: texture2d
        
        varying tex_coord1: vec2
        varying tex_coord2: vec2
        varying tex_coord3: vec2
        varying clipped: vec2
        varying pos: vec2
        
        fn vertex(self) -> vec4 {
            let min_pos = vec2(self.rect_pos.x, self.rect_pos.y)
            let max_pos = vec2(self.rect_pos.x + self.rect_size.x, self.rect_pos.y - self.rect_size.y)
            
            self.clipped = clamp(
                mix(min_pos, max_pos, self.geom_pos),
                self.draw_clip.xy,
                self.draw_clip.zw
            )
            
            let normalized: vec2 = (self.clipped - min_pos) / vec2(self.rect_size.x, -self.rect_size.y)
            
            self.tex_coord1 = mix(
                vec2(self.font_t1.x, 1.0-self.font_t1.y),
                vec2(self.font_t2.x, 1.0-self.font_t2.y),
                normalized.xy
            )
            self.pos = normalized;
            return self.camera_projection * (self.camera_view * (self.view_transform * vec4(
                self.clipped.x,
                self.clipped.y,
                self.char_depth + self.draw_zbias,
                1.
            )))
        }
        
        fn get_color(self) -> vec4 {
            return self.color;
        }
        fn blend_color(self, incol:vec4)->vec4{
            return incol
        }
        
        fn sample_color(self, scale:float, pos:vec2)->vec4{
            let s = sample2d(self.tex, pos).x;
            if (self.sdf_radius != 0.0) {
                // HACK(eddyb) harcoded atlas size (see asserts below).
                let texel_coords = pos.xy * 4096.0;
                s = clamp((s - (1.0 - self.sdf_cutoff)) * self.sdf_radius / scale + 0.5, 0.0, 1.0);
            } else {
                s = pow(s, self.curve);
            }
            let col = self.get_color(); 
            return self.blend_color(vec4(s * col.rgb * self.brightness * col.a, s * col.a));
        }
        
        fn pixel(self) -> vec4 {
            let texel_coords = self.tex_coord1.xy;
            let dxt = length(dFdx(texel_coords));
            let dyt = length(dFdy(texel_coords));
            let scale = (dxt + dyt) * 4096.0 *0.5;
            return self.sample_color(scale, self.tex_coord1.xy);
            // ok lets take our delta in the x direction
            /*
            //4x AA
            */
            /*
            let x1 = self.sample_color(scale, self.tex_coord1.xy);
            let x2 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * 0.5,0.0));
            let x3 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt* 0.5,dyt* 0.5));
            let x4 =  self.sample_color(scale, self.tex_coord1.xy+vec2(0.0,dyt* 0.5));
            return (x1+x2+x3+x4)/4;
            */
            /*
            let d = 0.333;
            let x1 = self.sample_color(scale, self.tex_coord1.xy);
            let x2 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * -d,0.0));
            let x3 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt* d,0.0));
            let x4 = self.sample_color(scale, self.tex_coord1.xy+vec2(0.0, dyt * -d));
            let x5 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * -d,dyt * -d));
            let x6 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt* d,dyt * -d));
            let x7 = self.sample_color(scale, self.tex_coord1.xy+vec2(0.0, dyt * d));
            let x8 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * -d,dyt * d));
            let x9 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt* d,dyt * d));
            return (x1+x2+x3+x4+x5+x6+x7+x8+x9)/9;
            */
            //16x AA
            /*
            let d = 0.25;
            let d2 = 0.5; 
            let d3 = 0.75; 
            let x1 = self.sample_color(scale, self.tex_coord1.xy);
            let x2 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d,0.0));
            let x3 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d2,0.0));
            let x4 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d3,0.0));
                        
            let x5 = self.sample_color(scale, self.tex_coord1.xy+vec2(0.0,dyt *d));
            let x6 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d,dyt *d));
            let x7 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d2,dyt *d));
            let x8 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d2,dyt *d));
                        
            let x9 = self.sample_color(scale, self.tex_coord1.xy+vec2(0.0,dyt *d2));
            let x10 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d,dyt *d2));
            let x11 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d2,dyt *d2));
            let x12 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d3,dyt *d2));           
            
            let x13 = self.sample_color(scale, self.tex_coord1.xy+vec2(0.0,dyt *d3));
            let x14 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d,dyt *d3));
            let x15 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d2,dyt *d3));
            let x16 =  self.sample_color(scale, self.tex_coord1.xy+vec2(dxt * d3,dyt *d3));            
            return (x1+x2+x3+x4+x5+x6+x7+x8+x9+x10+x11+x12+x13+x14+x15+x16)/16 ;*/
        }
    }
}

// HACK(eddyb) shader expects hardcoded atlas size (see `fn pixel` above).
const _: () = assert!(crate::font_atlas::ATLAS_WIDTH == 4096);
const _: () = assert!(crate::font_atlas::ATLAS_HEIGHT == 4096);

#[derive(Debug, Clone, Live, LiveHook, LiveRegister)]
#[live_ignore]
pub struct TextStyle {
    #[live()] pub font: Font,
    #[live(9.0)] pub font_size: f64,
    #[live(1.0)] pub brightness: f32,
    #[live(0.5)] pub curve: f32,
    #[live(1.4)] pub line_spacing: f64,
    #[live(1.1)] pub top_drop: f64,
    #[live(1.3)] pub height_factor: f64,
}

#[derive(Clone, Live, LiveHook)]
#[live_ignore]
pub enum TextWrap {
    #[pick] Ellipsis,
    Word,
    Line
}

struct WordIterator<'a> {
    char_iter: std::str::CharIndices<'a >,
    eval_width: f64,
    last_char: char,
    last_index: usize,
    font_size_total: f64,
    ignore_newlines: bool,
    combine_spaces: bool,
}
/*
struct WordIteratorItem {
    start: usize,
    end: usize,
    width: f64,
    with_new_line: bool
}*/

enum WordItem{
    Spaces{start:usize, end: usize, width: f64},
    Newline,
    Word{start:usize, end: usize, width: f64}
}

impl<'a> WordIterator<'a> {
    fn new(char_iter: std::str::CharIndices<'a>, eval_width: f64, font_size_total: f64, ignore_newlines:bool, combine_spaces:bool) -> Self {
        let mut s = Self {
            eval_width,
            char_iter: char_iter,
            last_char:'\0',
            last_index:0,
            font_size_total,
            ignore_newlines,
            combine_spaces
        };
        s.next_char();
        s
    }
    
    fn next_char(&mut self){
        if let Some((i, c)) = self.char_iter.next() {
            self.last_index = i;
            self.last_char = c;
        }
        else{
            self.last_index += self.last_char.len_utf8();
            self.last_char = '\0';
        };
    }
    
    fn next_word(&mut self, font: &mut CxFont) -> Option<WordItem> {
        if self.last_char == '\0'{
            return None
        }
        else if self.last_char == '\n'{ // return newline
            self.next_char();
            if self.ignore_newlines{
                return self.next_word(font);
            }
            return Some(WordItem::Newline);
        }
        else if self.last_char == ' '{
            let adv = if let Some(glyph) = font.get_glyph(' ') {
                glyph.horizontal_metrics.advance_width * self.font_size_total
            }else {0.0};
            let start = self.last_index;
            let mut width = 0.0;
            while self.last_char == ' '{
                if width + adv >= self.eval_width{
                    if start == self.last_index{// advance atleast one char
                        width += adv;
                        self.next_char();
                    }
                    break;
                }
                width += adv;
                self.next_char();
            }
            // lets make sure we advance atleast one char
            if self.combine_spaces{
                return Some(WordItem::Spaces{
                    start,
                    end: start+1,
                    width: adv,
                });
            }
            return Some(WordItem::Spaces{
                start,
                end: self.last_index,
                width,
            });
        }
        else{
            let start = self.last_index;
            let mut width = 0.0;
            while self.last_char != ' ' && self.last_char != '\0' && self.last_char != '\n' {
                let adv = if let Some(glyph) = font.get_glyph(self.last_char) {
                    glyph.horizontal_metrics.advance_width * self.font_size_total
                }else {0.0};
                if width + adv >= self.eval_width{
                    if start == self.last_index{// advance atleast one char
                        width += adv;
                        self.next_char();
                    }
                    break;
                }
                width += adv;
                self.next_char();
            }
            
            return Some(WordItem::Word{
                start,
                end: self.last_index,
                width,
            });
        }
        
    }
}

pub struct TextGeom {
    pub eval_width: f64,
    pub eval_height: f64,
    pub measured_width: f64,
    pub measured_height: f64,
    pub ellip_pt: Option<(usize, f64, usize)>
}

#[derive(Live, LiveRegister)]
#[repr(C)]
pub struct DrawText {
    #[rust] pub many_instances: Option<ManyInstances>,
    
    #[live] pub geometry: GeometryQuad2D,
    #[live] pub text_style: TextStyle,
    #[live] pub wrap: TextWrap,
    
    #[live] pub ignore_newlines: bool,
    #[live] pub combine_spaces: bool,
    
    #[live(1.0)] pub font_scale: f64,
    #[live(1.0)] pub draw_depth: f32,
    
    #[deref] pub draw_vars: DrawVars,
    // these values are all generated
    #[live] pub color: Vec4,
    #[calc] pub font_t1: Vec2,
    #[calc] pub font_t2: Vec2,
    #[calc] pub rect_pos: Vec2,
    #[calc] pub rect_size: Vec2,
    #[calc] pub draw_clip: Vec4,
    #[calc] pub char_depth: f32,
    #[calc] pub delta: Vec2,
    #[calc] pub shader_font_size: f32,
    #[calc] pub advance: f32,
}

impl LiveHook for DrawText {
    fn before_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        self.draw_vars.before_apply_init_shader(cx, apply, index, nodes, &self.geometry);
    }
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        self.draw_vars.after_apply_update_self(cx, apply, index, nodes, &self.geometry);
    }
}

impl DrawText {
    
    pub fn draw(&mut self, cx: &mut Cx2d, pos: DVec2, val: &str) {
        self.draw_inner(cx, pos, val, &mut *cx.fonts_atlas_rc.clone().0.borrow_mut());
        if self.many_instances.is_some() {
            self.end_many_instances(cx)
        }
    }
    
    pub fn draw_rel(&mut self, cx: &mut Cx2d, pos: DVec2, val: &str) {
        self.draw_inner(cx, pos + cx.turtle().origin(), val, &mut *cx.fonts_atlas_rc.clone().0.borrow_mut());
        if self.many_instances.is_some() {
            self.end_many_instances(cx)
        }
    }
    
    pub fn draw_abs(&mut self, cx: &mut Cx2d, pos: DVec2, val: &str) {
        self.draw_inner(cx, pos, val, &mut *cx.fonts_atlas_rc.clone().0.borrow_mut());
        if self.many_instances.is_some() {
            self.end_many_instances(cx)
        }
    }
    
    pub fn begin_many_instances(&mut self, cx: &mut Cx2d) {
        let fonts_atlas_rc = cx.fonts_atlas_rc.clone();
        let fonts_atlas = fonts_atlas_rc.0.borrow();
        self.begin_many_instances_internal(cx, &*fonts_atlas);
    }
    
    fn begin_many_instances_internal(&mut self, cx: &mut Cx2d, fonts_atlas: &CxFontAtlas) {
        self.update_draw_call_vars(fonts_atlas);
        let mi = cx.begin_many_aligned_instances(&self.draw_vars);
        self.many_instances = mi;
    }
    
    pub fn end_many_instances(&mut self, cx: &mut Cx2d) {
        if let Some(mi) = self.many_instances.take() {
            let new_area = cx.end_many_instances(mi);
            self.draw_vars.area = cx.update_area_refs(self.draw_vars.area, new_area);
        }
    }
    
    pub fn new_draw_call(&self, cx: &mut Cx2d) {
        cx.new_draw_call(&self.draw_vars);
    }
    
    pub fn update_draw_call_vars(&mut self, font_atlas: &CxFontAtlas) {
        self.draw_vars.texture_slots[0] = Some(font_atlas.texture_sdf.clone());
        self.draw_vars.user_uniforms[0] = self.text_style.brightness;
        self.draw_vars.user_uniforms[1] = self.text_style.curve;
        let (sdf_radius, sdf_cutoff) = font_atlas.alloc.sdf.as_ref()
            .map_or((0.0, 0.0), |sdf| (sdf.params.radius, sdf.params.cutoff));
        self.draw_vars.user_uniforms[2] = sdf_radius;
        self.draw_vars.user_uniforms[3] = sdf_cutoff;
    }
    
    fn draw_inner(&mut self, cx: &mut Cx2d, position: DVec2, chunk: &str, font_atlas: &mut CxFontAtlas) {
        let shape_cache_rc = cx.shape_cache_rc.clone();
        let mut shape_cache = shape_cache_rc.0.borrow_mut();
        let glyph_infos = shape_cache.shape(
            Direction::LeftToRight,
            chunk,
            &[self.text_style.font.font_id.unwrap()],
            font_atlas
        );
        self.draw_glyphs(cx, position, &glyph_infos, font_atlas);
    }

    
    pub fn compute_geom(&self, cx: &Cx2d, walk: Walk, text: &str) -> Option<TextGeom> {
        self.compute_geom_inner(cx, walk, text, &mut *cx.fonts_atlas_rc.0.borrow_mut())
    }
    
    fn compute_geom_inner(&self, cx: &Cx2d, walk: Walk, text: &str, fonts_atlas: &mut CxFontAtlas) -> Option<TextGeom> {
        // we include the align factor and the width/height
        let font_id = self.text_style.font.font_id.unwrap();
        
        if fonts_atlas.fonts[font_id].is_none() {
            return None
        }
        
        let font_size_logical = self.text_style.font_size * 96.0 / (72.0 * fonts_atlas.fonts[font_id].as_ref().unwrap().ttf_font.units_per_em);
        let line_height = self.text_style.font_size * self.text_style.height_factor * self.font_scale;
        let eval_width = cx.turtle().eval_width(walk.width, walk.margin, cx.turtle().layout().flow);
        let eval_height = cx.turtle().eval_height(walk.height, walk.margin, cx.turtle().layout().flow);
        
        match if walk.width.is_fit() {&TextWrap::Line}else {&self.wrap} {
            TextWrap::Ellipsis => {
                let ellip_width = if let Some(glyph) = fonts_atlas.fonts[font_id].as_mut().unwrap().get_glyph('.') {
                    glyph.horizontal_metrics.advance_width * font_size_logical * self.font_scale
                }
                else {
                    0.0
                };
                
                let mut measured_width = 0.0;
                let mut ellip_pt = None;
                for (i, c) in text.chars().enumerate() {
                    
                    if measured_width + ellip_width * 3.0 < eval_width {
                        ellip_pt = Some((i, measured_width, 3));
                    }
                    if let Some(glyph) = fonts_atlas.fonts[font_id].as_mut().unwrap().get_glyph(c) {
                        let adv = glyph.horizontal_metrics.advance_width * font_size_logical * self.font_scale;
                        // ok so now what.
                        if measured_width + adv >= eval_width { // we have to drop back to ellip_pt
                            // if we don't have an ellip_pt, set it to 0
                            if ellip_pt.is_none() {
                                let dots = if ellip_width * 3.0 < eval_width {3}
                                else if ellip_width * 2.0 < eval_width {2}
                                else if ellip_width < eval_width {1}
                                else {0};
                                ellip_pt = Some((0, 0.0, dots));
                            }
                            return Some(TextGeom {
                                eval_width,
                                eval_height,
                                measured_width: ellip_pt.unwrap().1 + ellip_width,
                                measured_height: line_height,
                                ellip_pt
                            })
                        }
                        measured_width += adv;
                    }
                }
                
                Some(TextGeom {
                    eval_width,
                    eval_height,
                    measured_width,
                    measured_height: line_height,
                    ellip_pt: None
                })
            }
            TextWrap::Word => {
                let mut max_width = 0.0;
                let mut measured_width = 0.0;
                let mut measured_height = line_height;
                
                let mut iter = WordIterator::new(
                    text.char_indices(),
                    eval_width, font_size_logical * self.font_scale,
                    self.ignore_newlines,
                    self.combine_spaces,
                );
                while let Some(word) = iter.next_word(fonts_atlas.fonts[font_id].as_mut().unwrap()) {
                    match word{
                        WordItem::Newline=>{
                            measured_height += line_height * self.text_style.line_spacing;
                            measured_width = 0.0;
                        }
                        WordItem::Spaces{width,..} | WordItem::Word{width,..}=>{
                            if measured_width + width >= eval_width {
                                measured_height += line_height * self.text_style.line_spacing;
                                measured_width = width;
                            }
                            else {
                                measured_width += width;
                            }
                            if measured_width > max_width {max_width = measured_width}
                        }
                    }
                }
                
                Some(TextGeom {
                    eval_width,
                    eval_height,
                    measured_width: max_width,
                    measured_height,
                    ellip_pt: None
                })
            }
            TextWrap::Line => {
                let mut max_width = 0.0;
                let mut measured_width = 0.0;
                let mut measured_height = line_height;
                
                for c in text.chars() {
                    if c == '\n' {
                        measured_height += line_height * self.text_style.line_spacing;
                    }
                    if let Some(glyph) = fonts_atlas.fonts[font_id].as_mut().unwrap().get_glyph(c) {
                        let adv = glyph.horizontal_metrics.advance_width * font_size_logical * self.font_scale;
                        measured_width += adv;
                    }
                    if measured_width > max_width {
                        max_width = measured_width;
                    }
                }
                Some(TextGeom {
                    eval_width,
                    eval_height,
                    measured_width: max_width,
                    measured_height: measured_height,
                    ellip_pt: None
                })
            }
        }
    }
    pub fn draw_walk_word(&mut self, cx: &mut Cx2d, text: &str){
        self.draw_walk_word_with(cx, text, |_,_|{});
    }
    
    pub fn draw_walk_word_with<F>(&mut self, cx: &mut Cx2d, text: &str, mut cb:F) where F: FnMut(&mut Cx2d, Rect){
        
        // this walks the turtle per word
        if text.len() == 0 {
            return
        }        
        let font_id = if let Some(font_id) = self.text_style.font.font_id{font_id}else{
            //log!("Draw text without font");
            return
        };
        let fonts_atlas_rc = cx.fonts_atlas_rc.clone();
        let mut fonts_atlas = fonts_atlas_rc.0.borrow_mut();
        let fonts_atlas = &mut*fonts_atlas;
                
        let font_size_logical = self.text_style.font_size * 96.0 / (72.0 * fonts_atlas.fonts[font_id].as_ref().unwrap().ttf_font.units_per_em);
        let line_drop = self.text_style.font_size * self.text_style.height_factor * self.font_scale * self.text_style.top_drop;
        
        // lets get the width of the current turtle
        // we need it for the next_word item to properly break off
        let padded_rect = cx.turtle().padded_rect();
        
        let mut iter = WordIterator::new(
            text.char_indices(),
            padded_rect.size.x,
            font_size_logical * self.font_scale, 
            self.ignore_newlines,
            self.combine_spaces,
        );
        let mut last_rect = None;
        while let Some(word) = iter.next_word(fonts_atlas.fonts[font_id].as_mut().unwrap()) {
            match word{
                WordItem::Newline=>{
                    cx.turtle_new_line();
                }
                WordItem::Spaces{start,end,width,..} | WordItem::Word{start,end,width,..}=>{
                    let walk_rect = cx.walk_turtle(Walk {
                        abs_pos: None,
                        margin: Margin::default(),
                        width: Size::Fixed(width),
                        height: Size::Fixed(line_drop)
                    });
                    if last_rect.is_none(){
                        last_rect = Some(walk_rect)
                    }
                    else{
                        let rect = last_rect.unwrap();
                        if walk_rect.pos.y > rect.pos.y { // we emit the last rect
                            cb(cx, rect);
                            last_rect = Some(walk_rect);
                        }
                        else{
                            last_rect.as_mut().unwrap().size.x += walk_rect.size.x;
                        }
                    }
                    if let Some(rect) = last_rect{
                        cb(cx, rect);
                    }
                    // make sure our iterator uses the xpos from the turtle
                    self.draw_inner(cx, walk_rect.pos, &text[start..end], fonts_atlas);
                }
            }
        }
        if self.many_instances.is_some() {
            self.end_many_instances(cx)
        }
    }
    
    pub fn draw_walk(&mut self, cx: &mut Cx2d, walk: Walk, align: Align, text: &str) {
        if text.len() == 0 {
            return
        }        
        let font_id = if let Some(font_id) = self.text_style.font.font_id{font_id}else{
            //log!("Draw text without font");
            return
        };
        let fonts_atlas_rc = cx.fonts_atlas_rc.clone();
        let mut fonts_atlas = fonts_atlas_rc.0.borrow_mut();
        let fonts_atlas = &mut*fonts_atlas;
        
        let _font_size_logical = self.text_style.font_size * 96.0 / (72.0 * fonts_atlas.fonts[font_id].as_ref().unwrap().ttf_font.units_per_em);
        let line_height = self.text_style.font_size * self.text_style.height_factor * self.font_scale;
                
        //let in_many = self.many_instances.is_some();
        // lets compute the geom

        //if !in_many {
        //    self.begin_many_instances_internal(cx, fonts_atlas);
        //}
        if let Some(geom) = self.compute_geom_inner(cx, walk, text, fonts_atlas) {
            let height = if walk.height.is_fit() {
                geom.measured_height
            } else {
                geom.eval_height
            };
            let y_align = (height - geom.measured_height) * align.y;
            
            match if walk.width.is_fit() {&TextWrap::Line}else {&self.wrap} {
                TextWrap::Ellipsis => {
                    // otherwise we should check the ellipsis
                    if let Some((ellip, at_x, dots)) = geom.ellip_pt {
                        // ok so how do we draw this
                        let rect = cx.walk_turtle(Walk {
                            abs_pos: walk.abs_pos,
                            margin: walk.margin,
                            width: Size::Fixed(geom.eval_width),
                            height: Size::Fixed(height)
                        });
                        
                        // Ensure the chunk before the ellipsis is aligned down to a char boundary
                        let chunk = text.get(0..ellip).unwrap_or_else(|| {
                            let mut new_ellip = ellip.saturating_sub(1);
                            while new_ellip > 0 {
                                if let Some(s) = text.get(0..new_ellip) {
                                    return s;
                                }
                                new_ellip -= 1;
                            }
                            ""
                        });
                        self.draw_inner(cx, rect.pos + dvec2(0.0, y_align), chunk, fonts_atlas);
                        self.draw_inner(cx, rect.pos + dvec2(at_x, y_align), &"..."[0..dots], fonts_atlas);
                    }
                    else { // we might have space to h-align
                        let rect = cx.walk_turtle(Walk {
                            abs_pos: walk.abs_pos,
                            margin: walk.margin,
                            width: Size::Fixed(geom.eval_width),
                            height: Size::Fixed(
                                if walk.height.is_fit() {
                                    geom.measured_height
                                } else {
                                    geom.eval_height
                                }
                            )
                        });
                        let x_align = (geom.eval_width - geom.measured_width) * align.x;
                        self.draw_inner(cx, rect.pos + dvec2(x_align, y_align), text, fonts_atlas);
                    }
                }
                TextWrap::Word => {
                    self.draw_walk_wrap(cx, walk, text, fonts_atlas);
                }
                TextWrap::Line => {
                    // lets just output it and walk it
                    let rect = cx.walk_turtle(Walk {
                        abs_pos: walk.abs_pos,
                        margin: walk.margin,
                        width: Size::Fixed(geom.measured_width),
                        height: Size::Fixed(height)
                    });
                    // lets do our y alignment
                    let mut ypos = 0.0;
                    for line in text.split('\n') {
                        self.draw_inner(cx, rect.pos + dvec2(0.0, y_align + ypos), line, fonts_atlas);
                        ypos += line_height * self.text_style.line_spacing;
                    }
                    
                }
            }
        }
        
        if self.many_instances.is_some() {
            self.end_many_instances(cx)
        }
    }
    
    pub fn closest_offset(&self, cx: &Cx, newline_indexes: Vec<usize>, pos: DVec2) -> Option<usize> {
        let area = &self.draw_vars.area;
        
        if !area.is_valid(cx) {
            return None
        }

        let line_spacing = self.get_line_spacing();
        let rect_pos = area.get_read_ref(cx, live_id!(rect_pos), ShaderTy::Vec2).unwrap();
        let delta = area.get_read_ref(cx, live_id!(delta), ShaderTy::Vec2).unwrap();
        let advance = area.get_read_ref(cx, live_id!(advance), ShaderTy::Float).unwrap();

        let mut last_y = None;
        let mut newlines = 0;
        for i in 0..rect_pos.repeat {
            if newline_indexes.contains(&(i + newlines)) {
                newlines += 1;
            }

            let index = rect_pos.stride * i;
            let x = rect_pos.buffer[index + 0] as f64 - delta.buffer[index + 0] as f64;

            let y = rect_pos.buffer[index + 1] - delta.buffer[index + 1];
            if last_y.is_none() {last_y = Some(y)}
            let advance = advance.buffer[index + 0] as f64;
            if i > 0 && (y - last_y.unwrap()) > 0.001 && pos.y < last_y.unwrap() as f64 + line_spacing as f64 {
                return Some(i - 1 + newlines)
            }
            if pos.x < x + advance * 0.5 && pos.y < y as f64 + line_spacing as f64 {
                return Some(i + newlines)
            }
            last_y = Some(y)
        }
        return Some(rect_pos.repeat + newlines);
        
    }
    
    pub fn get_selection_rects(&self, cx: &Cx, newline_indexes: Vec<usize>, start: usize, end: usize, shift: DVec2, pad: DVec2) -> Vec<Rect> {
        let area = &self.draw_vars.area;
        
        if !area.is_valid(cx) {
            return Vec::new();
        }

        // Adjustments because of newlines characters (they are not in the buffers)
        let start_offset = newline_indexes.iter().filter(|&&i| i < start).count();
        let start = start - start_offset;
        let end_offset = newline_indexes.iter().filter(|&&i| i < end).count();
        let end = end - end_offset;
        
        let rect_pos = area.get_read_ref(cx, live_id!(rect_pos), ShaderTy::Vec2).unwrap();
        let delta = area.get_read_ref(cx, live_id!(delta), ShaderTy::Vec2).unwrap();
        let advance = area.get_read_ref(cx, live_id!(advance), ShaderTy::Float).unwrap();
        
        if rect_pos.repeat == 0 || start >= rect_pos.repeat{
            return Vec::new();
        }
        // alright now we go and walk from start to end and collect our selection rects
        
        let index = start * rect_pos.stride;
        let start_x = rect_pos.buffer[index + 0] - delta.buffer[index + 0]; // + advance.buffer[index + 0] * pos;
        let start_y = rect_pos.buffer[index + 1] - delta.buffer[index + 1];
        let line_spacing = self.get_line_spacing();
        let mut last_y = start_y;
        let mut min_x = start_x;
        let mut last_x = start_x;
        let mut last_advance = advance.buffer[index + 0];
        let mut out = Vec::new();
        for index in start..end {
            if index >= rect_pos.repeat{
                break;
            }
            let index = index * rect_pos.stride;
            let end_x = rect_pos.buffer[index + 0] - delta.buffer[index + 0];
            let end_y = rect_pos.buffer[index + 1] - delta.buffer[index + 1];
            last_advance = advance.buffer[index + 0];
            if end_y > last_y { // emit rect
                out.push(Rect {
                    pos: dvec2(min_x as f64, last_y as f64) + shift,
                    size: dvec2((last_x - min_x + last_advance) as f64, line_spacing) + pad
                });
                min_x = end_x;
                last_y = end_y;
            }
            last_x = end_x;
        }
        out.push(Rect {
            pos: dvec2(min_x as f64, last_y as f64) + shift,
            size: dvec2((last_x - min_x + last_advance) as f64, line_spacing) + pad
        });
        out
    }
    
    pub fn get_char_count(&self, cx: &Cx) -> usize {
        let area = &self.draw_vars.area;
        if !area.is_valid(cx) {
            return 0
        }
        let rect_pos = area.get_read_ref(cx, live_id!(rect_pos), ShaderTy::Vec2).unwrap();
        rect_pos.repeat
    }
    
    pub fn get_cursor_pos(&self, cx: &Cx, newline_indexes: Vec<usize>, pos: f32, index: usize) -> Option<DVec2> {
        let area = &self.draw_vars.area;
        
        if !area.is_valid(cx) {
            return None
        }
        // Adjustment because of newlines characters (they are not in the buffers)
        let index_offset = newline_indexes.iter().filter(|&&i| i < index).count();
        let (index, pos) = if newline_indexes.contains(&(index)){
            (index - index_offset - 1, pos + 1.0)
        } else {
            (index - index_offset, pos)
        };
        
        let rect_pos = area.get_read_ref(cx, live_id!(rect_pos), ShaderTy::Vec2).unwrap();
        let delta = area.get_read_ref(cx, live_id!(delta), ShaderTy::Vec2).unwrap();
        let advance = area.get_read_ref(cx, live_id!(advance), ShaderTy::Float).unwrap();
        
        if rect_pos.repeat == 0 {
            return None
        }
        if index >= rect_pos.repeat {
            // lets get the last one and advance
            let index = (rect_pos.repeat - 1) * rect_pos.stride;
            let x = rect_pos.buffer[index + 0] - delta.buffer[index + 0] + advance.buffer[index + 0];
            let y = rect_pos.buffer[index + 1] - delta.buffer[index + 1];
            Some(dvec2(x as f64, y as f64))
        }
        else {
            let index = index * rect_pos.stride;
            let x = rect_pos.buffer[index + 0] - delta.buffer[index + 0] + advance.buffer[index + 0] * pos;
            let y = rect_pos.buffer[index + 1] - delta.buffer[index + 1];
            Some(dvec2(x as f64, y as f64))
        }
    }
    
    pub fn get_line_spacing(&self) -> f64 {
        self.text_style.font_size * self.text_style.height_factor * self.font_scale * self.text_style.line_spacing
    }
    
    pub fn get_font_size(&self) -> f64 {
        self.text_style.font_size * self.font_scale
    }
    
    pub fn get_monospace_base(&self, cx: &Cx2d) -> DVec2 {
        let mut fonts_atlas = cx.fonts_atlas_rc.0.borrow_mut();
        if self.text_style.font.font_id.is_none() {
            return DVec2::default();
        }
        let font_id = self.text_style.font.font_id.unwrap();
        if fonts_atlas.fonts[font_id].is_none() {
            return DVec2::default();
        }
        let font = fonts_atlas.fonts[font_id].as_mut().unwrap();
        let slot = font.owned_font_face.with_ref( | face | face.glyph_index('!').map_or(0, | id | id.0 as usize));
        let glyph = font.get_glyph_by_id(slot).unwrap();
        
        //let font_size = if let Some(font_size) = font_size{font_size}else{self.font_size};
        DVec2 {
            x: glyph.horizontal_metrics.advance_width * (96.0 / (72.0 * font.ttf_font.units_per_em)),
            y: self.text_style.line_spacing
        }
    }
}

impl DrawText {
    fn draw_walk_wrap(
        &mut self,
        cx: &mut Cx2d,
        walk: Walk,
        text: &str,
        font_atlas: &mut CxFontAtlas,
    ) {
        let Some(font_id) = self.text_style.font.font_id else {
            return
        };

        let shape_cache_rc = cx.shape_cache_rc.clone();
        let mut shape_cache = shape_cache_rc.0.borrow_mut();
        let shape_cache = &mut *shape_cache;

        let mut glyph_infos = Vec::new();
        let mut word_infos = Vec::new();
        let mut line_infos = Vec::new();

        // println!("BEGIN TEXT");
        for (line_byte_start, line_byte_end) in line_byte_ranges(text) {
            let line = &text[line_byte_start..line_byte_end];
            // println!("BEGIN LINE {:?}", line);

            let word_info_start = word_infos.len();
            for (word_byte_start, word_byte_end) in word_byte_ranges(line) {
                let word_byte_start = line_byte_start + word_byte_start;
                let word_byte_end = line_byte_start + word_byte_end;
                let _word = &text[word_byte_start..word_byte_end];
                // println!("WORD {:?}", word);

                let glyph_info_start = glyph_infos.len();
                for glyph_info in shape_cache.shape(
                    Direction::LeftToRight,
                    &text[word_byte_start..word_byte_end],
                    &[font_id],
                    font_atlas
                ) {
                    glyph_infos.push(GlyphInfo {
                        font_id: glyph_info.font_id,
                        glyph_id: glyph_info.glyph_id,
                        byte_index: glyph_info.byte_index + word_byte_start,
                    });
                }

                let mut width = 0.0;
                for glyph_info in &glyph_infos[glyph_info_start..] {
                    // Use the font id to get the font from the font atlas.
                    let font = font_atlas.fonts[glyph_info.font_id].as_mut().unwrap();
    
                    // Compute the font size.
                    let font_size_in_points = self.text_style.font_size / font.ttf_font.units_per_em;
                    let font_size_in_inches = font_size_in_points / POINTS_PER_INCH;
                    let font_size_in_logical_pixels = font_size_in_inches * LOGICAL_PIXELS_PER_INCH;
    
                    // Use the glyph id to get the glyph from the font.
                    let glyph = font.owned_font_face.with_ref(|face| {
                        font.ttf_font.get_glyph_by_id(face, glyph_info.glyph_id as usize).unwrap()
                    });
    
                    // Compute the advance width.
                    let advance_width_in_font_units = glyph.horizontal_metrics.advance_width;
                    let advance_width_in_logical_pixels = advance_width_in_font_units * font_size_in_logical_pixels;

                    width += advance_width_in_logical_pixels * self.font_scale;
                }

                word_infos.push(WordInfo {
                    glyph_info_start,
                    glyph_info_end: glyph_infos.len(),
                    width,
                });
            }

            // Use the font id to get the font from the font atlas.
            let font = font_atlas.fonts[font_id].as_mut().unwrap();

            // Compute the font size.
            let font_size_in_points = self.text_style.font_size / font.ttf_font.units_per_em;
            let font_size_in_inches = font_size_in_points / POINTS_PER_INCH;
            let font_size_in_logical_pixels = font_size_in_inches * LOGICAL_PIXELS_PER_INCH;
                                
            // Compute the line height.
            let line_height_in_logical_pixels = font.ttf_font.units_per_em * font_size_in_logical_pixels;

            line_infos.push(LineInfo {
                word_info_start,
                word_info_end: word_infos.len(),
                height: line_height_in_logical_pixels * self.text_style.line_spacing * self.font_scale,
            });
            // println!("END LINE");
        }

        let mut x = 0.0;
        let mut y = 0.0;
        let width = cx.turtle().eval_width(walk.width, walk.margin, cx.turtle().layout().flow);
        for line_info in &line_infos {
            for word_info in &word_infos[line_info.word_info_start..line_info.word_info_end] {
                if x + word_info.width > width && x > 0.0 {
                    x = 0.0;
                    y += line_info.height;
                }
                x += word_info.width;
            }
            x = 0.0;
            y += line_info.height;
        }

        let rect = cx.walk_turtle(Walk {
            abs_pos: walk.abs_pos,
            margin: walk.margin,
            width: Size::Fixed(width),
            height: Size::Fixed(y),
        });

        let mut x = 0.0;
        let mut y = 0.0;
        for line_info in &line_infos {
            for word_info in &word_infos[line_info.word_info_start..line_info.word_info_end] {
                if x + word_info.width > width && x > 0.0 {
                    x = 0.0;
                    y += line_info.height;
                }
                self.draw_glyphs(
                    cx,
                    dvec2(rect.pos.x + x, rect.pos.y + y),
                    &glyph_infos[word_info.glyph_info_start..word_info.glyph_info_end],
                    font_atlas
                );
                x += word_info.width;
            }
            x = 0.0;
            y += line_info.height;
        }

        // println!("END TEXT");
    }

    fn draw_glyphs(
        &mut self,
        cx: &mut Cx2d,
        position: DVec2,
        glyph_infos: &[GlyphInfo],
        font_atlas: &mut CxFontAtlas,
    ) {
        if !self.draw_vars.can_instance() {
            return;
        }

        if position.x.is_infinite() || position.x.is_nan() {
            return;
        }

        if glyph_infos.is_empty() {
            return;
        }

        // Lock the instance buffer.
        if !self.many_instances.is_some() {
            self.begin_many_instances_internal(cx, font_atlas);
        }
        
        // Get the device pixel ratio.
        let device_pixel_ratio = cx.current_dpi_factor();

        // Compute the glyph padding.
        let glyph_padding_in_device_pixels = 2.0;
        let glyph_padding_in_logical_pixels = glyph_padding_in_device_pixels / device_pixel_ratio;
        self.char_depth = self.draw_depth;
        let Some(mi) = &mut self.many_instances else {
            return;
        };
        
        let mut position = position;
        for glyph_info in glyph_infos {
            // Use the font id to get the font from the font atlas.
            let font = font_atlas.fonts[glyph_info.font_id].as_mut().unwrap();

            // Compute the font size.
            let font_size_in_points = self.text_style.font_size / font.ttf_font.units_per_em;
            let font_size_in_inches = font_size_in_points / POINTS_PER_INCH;
            let font_size_in_logical_pixels = font_size_in_inches * LOGICAL_PIXELS_PER_INCH;
            let font_size_in_device_pixels = font_size_in_logical_pixels * device_pixel_ratio;

            // Use the glyph id to get the glyph from the font.
            let glyph = font.owned_font_face.with_ref(|face| {
                font.ttf_font.get_glyph_by_id(face, glyph_info.glyph_id as usize).unwrap()
            });

            // Compute the glyph position.
            let glyph_position_x_in_font_units = glyph.bounds.p_min.x;
            let glyph_position_x_in_logical_pixels = glyph_position_x_in_font_units * font_size_in_logical_pixels;
            let glyph_position_y_in_font_units = glyph.bounds.p_min.y;
            let glyph_position_y_in_logical_pixels = glyph_position_y_in_font_units * font_size_in_logical_pixels;

            // Compute the glyph size.
            let glyph_size_x_in_font_units = glyph.bounds.p_max.x - glyph.bounds.p_min.x;
            let glyph_size_x_in_device_pixels = glyph_size_x_in_font_units * font_size_in_device_pixels;
            let glyph_size_y_in_font_units = glyph.bounds.p_max.y - glyph.bounds.p_min.y;
            let glyph_size_y_in_device_pixels = glyph_size_y_in_font_units * font_size_in_device_pixels;

            // Compute the padded glyph size.
            let padded_glyph_size_x_in_device_pixels = if glyph_size_x_in_device_pixels == 0.0 {
                0.0
            } else {
                glyph_size_x_in_device_pixels.ceil() + glyph_padding_in_device_pixels * 2.0
            };
            let padded_glyph_size_x_in_logical_pixels = padded_glyph_size_x_in_device_pixels / device_pixel_ratio;
            let padded_glyph_size_y_in_device_pixels = if glyph_size_y_in_device_pixels == 0.0 {
                0.0
            } else {
                glyph_size_y_in_device_pixels.ceil() + glyph_padding_in_device_pixels * 2.0
            };
            let padded_glyph_size_y_in_logical_pixels = padded_glyph_size_y_in_device_pixels / device_pixel_ratio;

            // Compute the advance width.
            let advance_width_in_font_units = glyph.horizontal_metrics.advance_width;
            let advance_width_in_logical_pixels = advance_width_in_font_units * font_size_in_logical_pixels;

            // Use the font size in device pixels to get the atlas page id from the font.
            let atlas_page_id = font.get_atlas_page_id(font_size_in_device_pixels);

            // Use the atlas page id to get the atlas page from the font.
            let atlas_page = &mut font.atlas_pages[atlas_page_id];

            // Use the padded glyph size in device pixels to get the atlas glyph from the atlas page.
            let atlas_glyph = *atlas_page.atlas_glyphs.entry(glyph_info.glyph_id as usize).or_insert_with(|| {
                font_atlas
                    .alloc
                    .alloc_atlas_glyph(
                        padded_glyph_size_x_in_device_pixels,
                        padded_glyph_size_y_in_device_pixels,
                        CxFontsAtlasTodo {
                            font_id: glyph_info.font_id,
                            atlas_page_id,
                            glyph_id: glyph_info.glyph_id as usize,
                        }
                    )
            });

            // Compute the distance from the current position to the rect.
            let delta_x = glyph_position_x_in_logical_pixels * self.font_scale - glyph_padding_in_logical_pixels;
            let delta_y = -(glyph_position_y_in_logical_pixels * self.font_scale - glyph_padding_in_logical_pixels);

            let fudge = self.text_style.font_size * self.font_scale * self.text_style.top_drop;
            let delta_y = delta_y + fudge;

            // Compute the rect size.
            let rect_size_x = padded_glyph_size_x_in_logical_pixels * self.font_scale;
            let rect_size_y = padded_glyph_size_y_in_logical_pixels * self.font_scale;
            
            // Emit the instance data.
            self.font_t1 = atlas_glyph.t1;
            self.font_t2 = atlas_glyph.t2;
            self.char_depth = self.draw_depth;
            self.rect_pos = dvec2(position.x + delta_x, position.y + delta_y).into();
            self.rect_size = dvec2(rect_size_x, rect_size_y).into();
            self.delta.x = delta_x as f32;
            self.delta.y = delta_y as f32;
            self.advance = (advance_width_in_logical_pixels * self.font_scale) as f32;
            mi.instances.extend_from_slice(self.draw_vars.as_slice());

            self.char_depth += ZBIAS_STEP;
            
            // Advance to the next position.
            position.x += advance_width_in_logical_pixels * self.font_scale;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LineInfo {
    word_info_start: usize,
    word_info_end: usize,
    height: f64,
}

#[derive(Clone, Copy, Debug)]
struct WordInfo {
    glyph_info_start: usize,
    glyph_info_end: usize,
    width: f64,
}

fn line_byte_ranges(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    text
        .lines()
        .scan(0, |byte_start, line| {
            let byte_end = *byte_start + line.len();
            let byte_range = (*byte_start, byte_end);
            *byte_start = byte_end + 1;
            Some(byte_range)
        })
}

fn word_byte_ranges(line: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    unicode_linebreak::linebreaks(line)
        .map(|(byte_index, _)| byte_index)
        .scan(0, |byte_start, byte_end| {
            let byte_range = (*byte_start, byte_end);
            *byte_start = byte_end;
            Some(byte_range)
        })
}