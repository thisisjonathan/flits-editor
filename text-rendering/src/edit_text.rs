use std::cell::{Cell, RefCell};
use std::{marker::PhantomData, sync::Arc};

use crate::compat::{RenderContext, UpdateContext};
use crate::font::{FontLike as _, FontType, Glyph};
use crate::html::{FormatSpans, Layout, LayoutBox, LayoutContent, LayoutLine, TextFormat};
use crate::string::SwfStrExt as _;
use crate::tag_utils::SwfMovie;
use gc_arena::{Collect, RefLock};
use ruffle_render::commands::Command as RenderCommand;
use ruffle_render::commands::CommandHandler as _;
use ruffle_render::matrix::Matrix;
use ruffle_render::transform::Transform;
use swf::{Color, Rectangle, Twips};

/// The kind of autosizing behavior an `EditText` should have, if any
#[derive(Copy, Clone, Collect, Debug, PartialEq, Eq)]
#[collect(no_drop)]
pub enum AutoSizeMode {
    None,
    Left,
    //Center,
    //Right,
}

struct EditTextData<'gc> {
    /// The current intrinsic bounds of the text field.
    bounds: Cell<Rectangle<Twips>>,
    /// How many pixels right the text is offset by. 0-based index.
    hscroll: Cell<f64>,
    /// How many lines down the text is offset by. 1-based index.
    scroll: Cell<usize>,

    /// The underlying text format spans of the `EditText`.
    ///
    /// This is generated from HTML (with optional CSS) or set directly, and
    /// can be directly manipulated by ActionScript. It can also be raised to
    /// an equivalent HTML representation, as long as no stylesheet is present.
    ///
    /// It is lowered further into layout boxes, which are used for actual
    /// rendering.
    text_spans: RefCell<FormatSpans>,

    /// The calculated layout.
    layout: RefLock<Layout<'gc>>,

    phantom: PhantomData<&'gc ()>,
}
impl EditTextData<'_> {
    fn vertical_scroll_offset(&self) -> Twips {
        if self.scroll.get() > 1 {
            let layout = self.layout.borrow();
            let lines = layout.lines();

            if let Some(line_data) = lines.get(self.scroll.get() - 1) {
                line_data.offset_y()
            } else {
                Twips::ZERO
            }
        } else {
            Twips::ZERO
        }
    }
}

pub struct EditText<'gc>(EditTextData<'gc>);
impl<'gc> EditText<'gc> {
    //const ANY_NEWLINE: [char; 2] = ['\n', '\r'];

    // This seems to be OS-independent
    //const INPUT_NEWLINE: char = '\r';

    /// Gutter is the constant internal padding of a text field.
    /// It applies to each side and cannot be changed.
    ///
    /// See <https://open-flash.github.io/mirrors/as2-language-reference/TextFormat.html#getTextExtent()>.
    /// See <https://help.adobe.com/en_US/FlashPlatform/reference/actionscript/3/flash/text/TextLineMetrics.html>.
    const GUTTER: Twips = Twips::new(40);

    /// Creates a new `EditText` from an SWF `DefineEditText` tag.
    pub fn from_swf_tag(
        context: &mut UpdateContext<'gc>,
        swf_movie: Arc<SwfMovie>,
        swf_tag: swf::EditText,
    ) -> Self {
        let default_format = TextFormat::from_swf_tag(swf_tag.clone(), swf_movie.clone(), context);
        let encoding = swf_movie.encoding();
        let text = swf_tag.initial_text().unwrap_or_default().decode(encoding);

        let mut text_spans = if swf_tag.is_html() {
            FormatSpans::from_html(
                &text,
                default_format,
                None,
                swf_tag.is_multiline(),
                false,
                swf_movie.version(),
            )
        } else {
            FormatSpans::from_text(text.into_owned(), default_format)
        };

        if swf_tag.is_password() {
            text_spans.hide_text();
        }

        let autosize = if swf_tag.is_auto_size() {
            AutoSizeMode::Left
        } else {
            AutoSizeMode::None
        };

        // let font_type = if swf_tag.use_outlines() {
        //     FontType::Embedded
        // } else {
        //     FontType::Device
        // };

        let is_word_wrap = swf_tag.is_word_wrap();
        let content_width = if autosize == AutoSizeMode::None || is_word_wrap {
            Some(swf_tag.bounds().width() - Self::GUTTER * 2)
        } else {
            None
        };

        let layout = crate::html::lower_from_text_spans(
            &text_spans,
            context,
            swf_movie.clone(),
            content_width,
            !swf_tag.is_read_only(),
            is_word_wrap,
            FontType::Embedded,
        );

        /*let variable = if !swf_tag.variable_name().is_empty() {
            Some(swf_tag.variable_name().decode(encoding))
        } else {
            None
        };*/
        //let variable = variable.map(|s| context.strings.intern_wstr(s).into());

        // // We match the flags from the DefineEditText SWF tag.
        // let mut flags = EditTextFlag::from_bits_truncate(swf_tag.flags().bits());
        // // For extra flags, use some of the SWF tag bits that are unused after the text field is created.
        // flags &= EditTextFlag::SWF_FLAGS;
        // flags.set(
        //     EditTextFlag::HAS_BACKGROUND,
        //     flags.contains(EditTextFlag::BORDER),
        // );

        // // Selections are mandatory in AS3.
        // let selection = if swf_movie.is_action_script_3() {
        //     Some(TextSelection::for_position(text_spans.text().len()))
        // } else {
        //     None
        // };

        EditText(EditTextData {
            bounds: Cell::new(*swf_tag.bounds()),
            hscroll: Cell::new(0.0),
            scroll: Cell::new(1),
            text_spans: RefCell::new(text_spans),
            layout: RefLock::new(layout),
            phantom: PhantomData::default(),
        })
    }

    pub fn render_self(&self, context: &mut RenderContext) {
        self.apply_autosize_bounds();

        // TODO: don't always render
        /*if !context.is_offscreen && !self.world_bounds().intersects(&context.stage.view_bounds()) {
            // Off-screen; culled
            return;
        }*/

        fn is_transform_positive_scale_only(context: &mut RenderContext) -> bool {
            let Matrix { a, b, c, d, .. } = context.transform_stack.transform().matrix;
            b == 0.0 && c == 0.0 && a > 0.0 && d > 0.0
        }

        // EditText is not rendered if device font is used
        // and if it's rotated, sheared, or reflected.
        if self.is_device_font() && !is_transform_positive_scale_only(context) {
            return;
        }

        /*if self
            .0
            .flags
            .get()
            .intersects(EditTextFlag::BORDER | EditTextFlag::HAS_BACKGROUND)
        {
            let background_color = Some(self.0.background_color.get())
                .filter(|_| self.0.flags.get().contains(EditTextFlag::HAS_BACKGROUND));
            let border_color = Some(self.0.border_color.get())
                .filter(|_| self.0.flags.get().contains(EditTextFlag::BORDER));

            if self.is_device_font() {
                self.draw_device_text_box(
                    context,
                    self.0.bounds.get(),
                    background_color,
                    border_color,
                );
            } else {
                self.draw_text_box(context, self.0.bounds.get(), background_color, border_color);
            }
        }*/

        context.commands.push_mask();

        let mask_bounds = grow_x(self.0.bounds.get(), -Self::GUTTER);
        let mask = Matrix::create_box_from_rectangle(&mask_bounds);

        context.commands.draw_rect(
            Color::WHITE,
            context.transform_stack.transform().matrix * mask,
        );
        context.commands.activate_mask();

        context.transform_stack.push(&Transform {
            matrix: self.layout_to_local_matrix(),
            ..Default::default()
        });

        let mut render_state = Default::default();
        self.render_text(context, &mut render_state);

        /*self.render_debug_boxes(
            context,
            self.0.layout_debug_boxes_flags.get(),
            &self.0.layout.borrow(),
        );*/

        context.transform_stack.pop();

        context.commands.deactivate_mask();
        context.commands.draw_rect(
            Color::WHITE,
            context.transform_stack.transform().matrix * mask,
        );
        context.commands.pop_mask();

        if let Some(draw_caret_command) = render_state.draw_caret_command {
            context.commands.commands.push(draw_caret_command);
        }
    }

    /// Render the visible text along with selection and the caret.
    fn render_text(&self, context: &mut RenderContext<'_>, render_state: &mut EditTextRenderState) {
        //self.render_selection_background(context);
        self.render_lines(context, |context, line| {
            self.render_layout_line(context, line, render_state);
        });
    }

    /// Render lines according to the given procedure.
    ///
    /// This skips invisible lines.
    fn render_lines<F>(&self, context: &mut RenderContext<'_>, mut f: F)
    where
        F: FnMut(&mut RenderContext<'_>, &LayoutLine<'gc>),
    {
        // Skip lines that are off-screen.
        let lines_to_skip = self.scroll().saturating_sub(1);
        for line in self.0.layout.borrow().lines().iter().skip(lines_to_skip) {
            f(context, line);
        }
    }

    fn render_layout_line(
        &self,
        context: &mut RenderContext<'_>,
        line: &LayoutLine<'gc>,
        render_state: &mut EditTextRenderState,
    ) {
        let max_descent = line.descent();
        for layout_box in line.boxes_iter() {
            self.render_layout_box(context, layout_box, render_state, max_descent);
        }
    }

    /// Render a layout box, plus its children.
    fn render_layout_box(
        &self,
        context: &mut RenderContext<'_>,
        lbox: &LayoutBox<'gc>,
        _render_state: &mut EditTextRenderState,
        max_descent: Twips,
    ) {
        let origin = lbox.bounds().origin();

        // If text's top is under the textbox's bottom, skip drawing.
        // TODO: FP actually skips drawing a line as soon as its bottom is under the textbox;
        //   Current logic is conservative for safety (and even of this I'm not 100% sure).
        //   (maybe we could cull-before-render all glyphs, thus removing the need for masking?)
        // [KJ] FP always displays the first visible line (sometimes masked, sometimes sticking out of bounds),
        //      culls any other line which is not fully visible; masking is always used for left/right bounds
        // TODO: also cull text that's simply out of screen, just like we cull whole DOs in render_self().
        if origin.y() + Self::GUTTER - self.0.vertical_scroll_offset()
            > self.0.bounds.get().height()
        {
            return;
        }

        context.transform_stack.push(&Transform {
            matrix: Matrix::translate(origin.x(), origin.y()),
            ..Default::default()
        });

        //let visible_selection = self.visible_selection();

        /*let caret = if let LayoutContent::Text { start, end, .. } = &lbox.content() {
            if let Some(visible_selection) = visible_selection {
                let text_len = self.0.text_spans.borrow().text().len();
                if visible_selection.is_caret()
                    && !self.0.flags.get().contains(EditTextFlag::READ_ONLY)
                    && visible_selection.start() >= *start
                    && (visible_selection.end() < *end || *end == text_len)
                    && !visible_selection.blinks_now()
                {
                    Some(visible_selection.start() - start)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };*/

        /*let start = if let LayoutContent::Text { start, .. } = &lbox.content() {
            *start
        } else {
            0
        };*/

        // If the font can't be found or has no glyph information, use the "device font" instead.
        // We're cheating a bit and not actually rendering text using the OS/web.
        // Instead, we embed an SWF version of Noto Sans to use as the "device font", and render
        // it the same as any other SWF outline text.
        if let Some((text, _tf, font, params, color)) =
            lbox.as_renderable_text(self.0.text_spans.borrow().displayed_text())
        {
            let baseline = font.get_baseline_for_height(params.height());
            //let descent = font.get_descent_for_height(params.height());
            let baseline_adjustment = baseline - params.height();
            //let caret_height = baseline + descent;
            //let mut caret_x = Twips::ZERO;
            font.evaluate(
                text,
                self.text_transform(color, baseline_adjustment),
                params,
                |_pos, transform, glyph: &Glyph, _advance, _x| {
                    if let Some(glyph_shape_handle) = glyph.shape_handle(context.renderer) {
                        // If it's highlighted, override the color.
                        /*if matches!(visible_selection, Some(visible_selection) if visible_selection.contains(start + pos)) {
                            // Set text color to white
                            context.transform_stack.push(&Transform {
                                matrix: transform.matrix,
                                color_transform: ColorTransform::IDENTITY,
                                //perspective_projection: transform.perspective_projection,
                            });
                        } else {*/
                        context.transform_stack.push(transform);
                        //}

                        // Render glyph.
                        context
                            .commands
                            .render_shape(glyph_shape_handle, context.transform_stack.transform());
                        context.transform_stack.pop();
                    }

                    // Update caret position
                    /*if let Some(caret) = caret {
                        if pos == caret {
                            caret_x = x;
                        } else if caret > 0 && pos == caret - 1 {
                            // The caret may be rendered at the end, after all glyphs.
                            caret_x = x + advance;
                        }
                    }*/
                },
            );

            /*if caret.is_some() {
                self.render_caret(context, caret_x, caret_height, color, render_state);
            }*/

            if let LayoutContent::Text {
                underline: true, ..
            } = lbox.content()
            {
                // Draw underline
                let underline_y = baseline + (max_descent / 2);
                let underline_width = lbox.bounds().width();
                self.render_underline(context, underline_width, underline_y, color);
            }
        }

        /*if let Some(drawing) = lbox.as_renderable_drawing() {
            drawing.render(context);
        }*/

        context.transform_stack.pop();
    }

    fn render_underline(
        &self,
        context: &mut RenderContext<'_>,
        width: Twips,
        y: Twips,
        color: Color,
    ) {
        let underline = context.transform_stack.transform().matrix
            * Matrix::create_box_with_rotation(width.to_pixels() as f32, 1.0, 0.0, Twips::ZERO, y);

        // TODO?
        //let pixel_snapping = EditTextPixelSnapping::new(context.stage.quality());
        //pixel_snapping.apply(&mut underline);

        context.commands.draw_line(color, underline);
    }

    /// Construct a base text transform for a particular `EditText` span.
    ///
    /// This `text_transform` is separate from and relative to the base
    /// transform that this `EditText` automatically gets by virtue of being a
    /// `DisplayObject`.
    pub fn text_transform(&self, color: Color, baseline_adjustment: Twips) -> Transform {
        let mut transform: Transform = Default::default();
        transform.color_transform.set_mult_color(&color);

        // TODO MIKE: This feels incorrect here but is necessary for correct vertical position;
        // the glyphs are rendered relative to the baseline. This should be taken into account either
        // by the layout code earlier (cursor should start at the baseline, not 0,0) and/or by
        // font.evaluate (should return transforms relative to the baseline).
        transform.matrix.ty = baseline_adjustment;

        transform
    }

    fn is_device_font(&self) -> bool {
        false
    }

    /// Returns the matrix for transforming from layout
    /// coordinate space into this object's local space.
    fn layout_to_local_matrix(&self) -> Matrix {
        let bounds = self.0.bounds.get();
        Matrix::translate(
            bounds.x_min + Self::GUTTER - Twips::from_pixels(self.0.hscroll.get()),
            bounds.y_min + Self::GUTTER - self.0.vertical_scroll_offset(),
        )
    }

    fn apply_autosize_bounds(&self) {
        // TODO
    }

    pub fn scroll(&self) -> usize {
        self.0.scroll.get()
    }
}

#[derive(Clone, Debug, Default)]
struct EditTextRenderState {
    /// Used for delaying rendering the caret, so that it's
    /// rendered outside of the text mask.
    draw_caret_command: Option<RenderCommand>,
}

// seperate because the version of Ruffle that Flits Editor uses doesn't have this function yet
#[must_use]
pub fn grow_x(mut rect: Rectangle<Twips>, amount: Twips) -> Rectangle<Twips> {
    if rect.is_valid() {
        rect.x_min -= amount;
        rect.x_max += amount;
    }
    rect
}
