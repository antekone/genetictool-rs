/// Licencja: MIT

mod options_window;
use options_window::{OptionsParams, OptionsWindow};

use eframe::egui;
use egui::{vec2, Color32, FontId, Layout, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2, Widget};
use rand::Rng;
use rand::SeedableRng;
use std::sync::{Arc, Mutex};

/// Liczba bitów kodujących jeden chromosom (chromosom = wartość X).
/// 16 bitów daje rozdzielczość ~0.0003 na dziedzinie [-10, 10].
const BITS: usize = 16;

/// Pojedynczy chromosom: ciąg bitów reprezentujący wartość X z dziedziny funkcji.
///
/// Bity interpretowane są jako liczba całkowita bez znaku [0, 2^BITS),
/// a następnie liniowo mapowane na przedział [x_min, x_max].
#[derive(Clone, Debug)]
struct Chromosome {
    /// Geny – ciąg `BITS` bitów.
    genes: [bool; BITS],
    /// Wartość fitness (f(x)) obliczona dla tego chromosomu.
    fitness: f64,
    /// Wartość X zdekodowana z genów.
    x: f64,
}

impl Chromosome {
    /// Tworzy chromosom z losowych bitów w dziedzinie [x_min, x_max].
    fn random<R: Rng>(x_min: f64, x_max: f64, rng: &mut R) -> Self {
        let mut genes = [false; BITS];
        for bit in genes.iter_mut() {
            *bit = rng.gen_bool(0.5);
        }
        let x = Self::decode(&genes, x_min, x_max);
        Self { genes, fitness: 0.0, x }
    }

    /// Dekoduje ciąg bitów na wartość X w dziedzinie [x_min, x_max].
    fn decode(genes: &[bool; BITS], x_min: f64, x_max: f64) -> f64 {
        let max_val = ((1u64 << BITS) - 1) as f64;
        let int_val: u64 = genes.iter().fold(0u64, |acc, &b| (acc << 1) | b as u64);
        x_min + (int_val as f64 / max_val) * (x_max - x_min)
    }

    /// Oblicza i zapisuje fitness dla podanej funkcji celu.
    fn evaluate(&mut self, func: fn(f64) -> f64) {
        self.fitness = func(self.x);
    }

    /// Zwraca czytelny podgląd: bity (pierwsze 8 skrócone) + x + fitness.
    fn display_str(&self) -> String {
        let bits: String = self.genes.iter().map(|&b| if b { '1' } else { '0' }).collect();
        format!("{}  x={:7.4}  f={:8.4}", bits, self.x, self.fitness)
    }
}

// ---------------------------------------------------------------------------

/// Cała populacja: zbiór chromosomów + metadane bieżącego pokolenia.
#[derive(Clone, Debug)]
struct Population {
    chromosomes: Vec<Chromosome>,
    /// Numer aktualnego pokolenia (0 = populacja startowa).
    generation: usize,
}

impl Population {
    /// Tworzy losową populację startową.
    fn random(size: usize, x_min: f64, x_max: f64, func: fn(f64) -> f64) -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(12345);
        let mut chromosomes: Vec<Chromosome> = (0..size)
            .map(|_| {
                let mut c = Chromosome::random(x_min, x_max, &mut rng);
                c.evaluate(func);
                c
            })
            .collect();

        // Sortujemy malejąco po fitness – najlepszy na górze listy.
        chromosomes.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());
        Self { chromosomes, generation: 0 }
    }

    /// Zwraca najlepszy chromosom (po sortowaniu zawsze pierwszy).
    fn best(&self) -> Option<&Chromosome> {
        self.chromosomes.first()
    }
}

// ---------------------------------------------------------------------------
// Kolory używane w całym wykresie
// ---------------------------------------------------------------------------
struct PlotColors {
    bg:         Color32,
    margin:     Color32,
    grid:       Color32,
    axis:       Color32,
    tick:       Color32,
    label:      Color32,
    curve:      Color32,
    crosshair:  Color32,
    crosshair_bg: Color32,
}

impl PlotColors {
    fn default_dark() -> Self {
        Self {
            bg:           Color32::from_gray(20),
            margin:       Color32::from_gray(14),
            grid:         Color32::from_gray(40),
            axis:         Color32::from_gray(110),
            tick:         Color32::from_gray(160),
            label:        Color32::from_gray(200),
            curve:        Color32::from_rgb(80, 200, 120),
            crosshair:    Color32::from_rgb(255, 220, 80),
            crosshair_bg: Color32::from_rgba_premultiplied(0, 0, 0, 180),
        }
    }

    fn default_light() -> Self {
        Self {
            bg:           Color32::from_gray(250),
            margin:       Color32::from_gray(240),
            grid:         Color32::from_gray(220),
            axis:         Color32::from_gray(100),
            tick:         Color32::from_gray(60),
            label:        Color32::from_gray(30),
            curve:        Color32::from_rgb(0, 120, 60),
            crosshair:    Color32::from_rgb(200, 100, 0),
            crosshair_bg: Color32::from_rgba_premultiplied(255, 255, 255, 200),
        }
    }
}

// ---------------------------------------------------------------------------
// Wstępnie obliczone dane układu / skali
// ---------------------------------------------------------------------------
struct PlotLayout {
    rect:       Rect,
    plot_rect:  Rect,
    font_size:  f32,
    font:       FontId,
    tick_len:   f32,
    x_min: f64, x_max: f64,
    y_min: f64, y_max: f64,
    x_step: f64, y_step: f64,
    x_ticks: Vec<f64>,
    y_ticks: Vec<f64>,
}

impl PlotLayout {
    fn new(rect: Rect, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        let h = rect.height();
        let font_size = (h * 0.028).clamp(9.0, 13.0);
        let font      = FontId::monospace(font_size);
        let tick_len  = (font_size * 0.5).ceil();

        let margin_left   = ((7.0_f32) * font_size * 0.62).ceil();
        let margin_bottom = (font_size * 1.6).ceil();

        let plot_rect = Rect::from_min_max(
            rect.left_top()     + vec2(margin_left, 0.0),
            rect.right_bottom() - vec2(0.0, margin_bottom),
        );

        let pw = plot_rect.width();
        let ph = plot_rect.height();

        let x_span = x_max - x_min;
        let y_span = y_max - y_min;

        let x_target = ((pw / (font_size * 6.0)) as f64).clamp(2.0, 20.0);
        let y_target = ((ph / (font_size * 2.8)) as f64).clamp(2.0, 20.0);

        let x_step = nice_step(x_span, x_target);
        let y_step = nice_step(y_span, y_target);

        let x_ticks = ticks_for(x_min, x_max, x_step);
        let y_ticks = ticks_for(y_min, y_max, y_step);

        Self {
            rect, plot_rect, font_size, font, tick_len,
            x_min, x_max, y_min, y_max, x_step, y_step,
            x_ticks, y_ticks,
        }
    }

    fn to_screen(&self, x: f64, y: f64) -> Pos2 {
        let pw = self.plot_rect.width()  as f64;
        let ph = self.plot_rect.height() as f64;
        let px = (x - self.x_min) / (self.x_max - self.x_min);
        let py = 1.0 - (y - self.y_min) / (self.y_max - self.y_min);
        self.plot_rect.left_top() + vec2((px * pw) as f32, (py * ph) as f32)
    }

    fn x_to_screen(&self, x: f64) -> f32 {
        let pw = self.plot_rect.width() as f64;
        self.plot_rect.left() + ((x - self.x_min) / (self.x_max - self.x_min) * pw) as f32
    }

    fn y_to_screen(&self, y: f64) -> f32 {
        let ph = self.plot_rect.height() as f64;
        self.plot_rect.top()
            + ((1.0 - (y - self.y_min) / (self.y_max - self.y_min)) * ph) as f32
    }
}

// ---------------------------------------------------------------------------
// Funkcje pomocnicze
// ---------------------------------------------------------------------------

fn nice_step(span: f64, target_count: f64) -> f64 {
    let raw   = span / target_count;
    let mag   = raw.log10().floor();
    let scale = 10f64.powf(mag);
    let norm  = raw / scale;
    let nice  = if norm < 1.5      { 1.0 }
                else if norm < 3.0 { 2.0 }
                else if norm < 4.0 { 2.5 }
                else if norm < 7.5 { 5.0 }
                else               { 10.0 };
    nice * scale
}

fn ticks_for(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let first = (lo / step).ceil() * step;
    let mut v = Vec::new();
    let mut t = first;
    while t <= hi + step * 1e-9 {
        if t >= lo - step * 1e-9 { v.push(t); }
        t += step;
    }
    v
}

fn fmt_tick(v: f64, step: f64) -> String {
    let decimals = ((-step.log10().floor()).max(0.0) as usize).min(4);
    if decimals == 0 { format!("{:.0}", v) }
    else             { format!("{:.prec$}", v, prec = decimals) }
}

fn draw_dashed_line(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dash = 4.0_f32;
    let gap  = 4.0_f32;
    let delta = to - from;
    let len   = delta.length();
    if len < 1.0 { return; }
    let dir = delta / len;
    let mut d = 0.0_f32;
    while d < len {
        let d_end = (d + dash).min(len);
        painter.line_segment([from + dir * d, from + dir * d_end], stroke);
        d += dash + gap;
    }
}

fn draw_background(painter: &Painter, layout: &PlotLayout, colors: &PlotColors) {
    painter.rect_filled(layout.rect,      0.0, colors.margin);
    painter.rect_filled(layout.plot_rect, 0.0, colors.bg);
}

fn draw_grid(painter: &Painter, layout: &PlotLayout, colors: &PlotColors) {
    let stroke = Stroke::new(1.0, colors.grid);
    for &xv in &layout.x_ticks {
        let sx = layout.x_to_screen(xv);
        painter.line_segment(
            [Pos2::new(sx, layout.plot_rect.top()), Pos2::new(sx, layout.plot_rect.bottom())],
            stroke,
        );
    }
    for &yv in &layout.y_ticks {
        let sy = layout.y_to_screen(yv);
        painter.line_segment(
            [Pos2::new(layout.plot_rect.left(), sy), Pos2::new(layout.plot_rect.right(), sy)],
            stroke,
        );
    }
}

fn draw_zero_axes(painter: &Painter, layout: &PlotLayout, colors: &PlotColors) {
    let stroke = Stroke::new(1.0, colors.axis);
    if layout.y_min <= 0.0 && layout.y_max >= 0.0 {
        let sy = layout.y_to_screen(0.0);
        painter.line_segment(
            [Pos2::new(layout.plot_rect.left(), sy), Pos2::new(layout.plot_rect.right(), sy)],
            stroke,
        );
    }
    if layout.x_min <= 0.0 && layout.x_max >= 0.0 {
        let sx = layout.x_to_screen(0.0);
        painter.line_segment(
            [Pos2::new(sx, layout.plot_rect.top()), Pos2::new(sx, layout.plot_rect.bottom())],
            stroke,
        );
    }
}

fn draw_ticks_and_labels(painter: &Painter, layout: &PlotLayout, colors: &PlotColors) {
    let tick_stroke = Stroke::new(1.0, colors.tick);
    let tl          = layout.tick_len;
    for &xv in &layout.x_ticks {
        let sx = layout.x_to_screen(xv);
        painter.line_segment(
            [Pos2::new(sx, layout.plot_rect.bottom()), Pos2::new(sx, layout.plot_rect.bottom() + tl)],
            tick_stroke,
        );
        painter.text(
            Pos2::new(sx, layout.plot_rect.bottom() + tl + 1.0),
            egui::Align2::CENTER_TOP,
            fmt_tick(xv, layout.x_step),
            layout.font.clone(),
            colors.label,
        );
    }
    for &yv in &layout.y_ticks {
        let sy = layout.y_to_screen(yv);
        painter.line_segment(
            [Pos2::new(layout.plot_rect.left() - tl, sy), Pos2::new(layout.plot_rect.left(), sy)],
            tick_stroke,
        );
        painter.text(
            Pos2::new(layout.plot_rect.left() - tl - 2.0, sy),
            egui::Align2::RIGHT_CENTER,
            fmt_tick(yv, layout.y_step),
            layout.font.clone(),
            colors.label,
        );
    }
}

fn draw_curve(painter: &Painter, layout: &PlotLayout, colors: &PlotColors, eval: impl Fn(f64) -> f64) {
    let stroke = Stroke::new(1.5, colors.curve);
    let cols   = layout.plot_rect.width() as usize;
    let x_span = layout.x_max - layout.x_min;
    let mut prev: Option<Pos2> = None;
    for col in 0..cols {
        let t = col as f64 / (cols - 1).max(1) as f64;
        let x = layout.x_min + t * x_span;
        let y = eval(x);
        if y.is_finite() {
            let p = layout.to_screen(x, y);
            if let Some(prev_p) = prev {
                painter.line_segment([prev_p, p], stroke);
            }
            prev = Some(p);
        } else {
            prev = None;
        }
    }
}

/// Rysuje punkty populacji jako pionowe kreski na krzywej.
fn draw_population_on_curve(
    painter: &Painter,
    layout: &PlotLayout,
    population: &Population,
    _colors: &PlotColors,
) {
    // Najlepszy chromosom – złota gwiazdka, reszta – niebieskie krople.
    for (i, chrom) in population.chromosomes.iter().enumerate() {
        let x = chrom.x;
        let y = chrom.fitness;
        if !x.is_finite() || !y.is_finite() { continue; }
        // Rysuj tylko jeśli mieści się w bieżącym zakresie osi.
        if x < layout.x_min || x > layout.x_max { continue; }
        if y < layout.y_min || y > layout.y_max { continue; }

        let p = layout.to_screen(x, y);
        let (color, radius) = if i == 0 {
            // Najlepszy chromosom - pomarańczowy/złoty
            (Color32::from_rgb(220, 140, 0), 5.0_f32)
        } else {
            // Reszta - niebieski dostosowany do motywu
            (Color32::from_rgb(60, 120, 200), 3.0_f32)
        };
        painter.circle_filled(p, radius, color);
    }
}

fn draw_crosshair(painter: &Painter, layout: &PlotLayout, colors: &PlotColors, hx: f64, hy: f64) {
    let sx = layout.x_to_screen(hx);
    let sy = layout.y_to_screen(hy);
    let center = Pos2::new(sx, sy);
    let pr = layout.plot_rect;
    let tl = layout.tick_len;

    // Kolor przerywanych linii dostosowany do motywu
    let dot_stroke = Stroke::new(1.0, colors.grid);
    draw_dashed_line(painter, Pos2::new(pr.left(), sy), Pos2::new(sx, sy),          dot_stroke);
    draw_dashed_line(painter, Pos2::new(sx, sy),        Pos2::new(pr.right(), sy),  dot_stroke);
    draw_dashed_line(painter, Pos2::new(sx, pr.top()),  Pos2::new(sx, sy),          dot_stroke);
    draw_dashed_line(painter, Pos2::new(sx, sy),        Pos2::new(sx, pr.bottom()), dot_stroke);

    let cross_stroke = Stroke::new(1.5, colors.crosshair);
    let arm = 5.0_f32;
    painter.line_segment([center - vec2(arm, 0.0), center + vec2(arm, 0.0)], cross_stroke);
    painter.line_segment([center - vec2(0.0, arm), center + vec2(0.0, arm)], cross_stroke);

    let font = FontId::monospace(layout.font_size);
    let color = colors.crosshair;
    let bg = colors.crosshair_bg;

    let x_label = format!("{:.3}", hx);
    let x_label_pos = Pos2::new(sx, pr.bottom() + tl + 1.0);
    let x_galley = painter.layout_no_wrap(x_label.clone(), font.clone(), color);
    let x_bg = Rect::from_center_size(
        x_label_pos + vec2(0.0, x_galley.size().y * 0.5),
        x_galley.size() + vec2(4.0, 2.0),
    );
    painter.rect_filled(x_bg, 2.0, bg);
    painter.text(x_label_pos, egui::Align2::CENTER_TOP, x_label, font.clone(), color);

    let y_label = format!("{:.3}", hy);
    let y_label_pos = Pos2::new(pr.left() - tl - 2.0, sy);
    let y_galley = painter.layout_no_wrap(y_label.clone(), font.clone(), color);
    let y_bg = Rect::from_center_size(
        y_label_pos - vec2(y_galley.size().x * 0.5 + 2.0, 0.0),
        y_galley.size() + vec2(4.0, 2.0),
    );
    painter.rect_filled(y_bg, 2.0, bg);
    painter.text(y_label_pos, egui::Align2::RIGHT_CENTER, y_label, font, color);
}

// ---------------------------------------------------------------------------
// FunctionPlot
// ---------------------------------------------------------------------------
struct FunctionPlot {
    func:  fn(f64) -> f64,
    x_min: f64,
    x_max: f64,
}

impl FunctionPlot {
    fn new(func: fn(f64) -> f64, x_min: f64, x_max: f64) -> Self {
        Self { func, x_min, x_max }
    }

    /// Funkcja celu – do maksymalizacji przez GA.
    fn target(x: f64) -> f64 {
        (x + 5.0) * (2.0 * x - 5.0).cos() - 5.0
    }

    fn eval(&self, x: f64) -> f64 {
        (self.func)(x)
    }

    fn y_range(&self, steps: usize) -> (f64, f64) {
        let mut y_min = f64::MAX;
        let mut y_max = f64::MIN;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = self.x_min + t * (self.x_max - self.x_min);
            let y = self.eval(x);
            if y.is_finite() {
                y_min = y_min.min(y);
                y_max = y_max.max(y);
            }
        }
        let pad = (y_max - y_min) * 0.1;
        (y_min - pad, y_max + pad)
    }

    fn paint(&self,
        ui: &Ui,
        painter: &Painter,
        rect: Rect,
        hover: Option<(f64, f64)>,
        population: Option<&Population>
    ) {
        if rect.width() < 4.0 || rect.height() < 4.0 { return; }

        let cols = rect.width() as usize;
        let (y_min, y_max) = self.y_range(cols * 4);
        let layout = PlotLayout::new(rect, self.x_min, self.x_max, y_min, y_max);

        // Automatyczne wykrywanie motywu z egui
        let colors = if ui.visuals().dark_mode {
            PlotColors::default_dark()
        } else {
            PlotColors::default_light()
        };

        if layout.plot_rect.width() < 2.0 || layout.plot_rect.height() < 2.0 {
            return;
        }

        draw_background(painter, &layout, &colors);
        draw_grid(painter, &layout, &colors);
        draw_zero_axes(painter, &layout, &colors);
        draw_ticks_and_labels(painter, &layout, &colors);
        draw_curve(painter, &layout, &colors, |x| self.eval(x));

        // Rysuj populację na krzywej (jeśli istnieje).
        if let Some(pop) = population {
            draw_population_on_curve(painter, &layout, pop, &colors);
        }

        if let Some((hx, hy)) = hover {
            draw_crosshair(painter, &layout, &colors, hx, hy);
        }
    }
}

// ---------------------------------------------------------------------------
// FunctionPlotWidget
// ---------------------------------------------------------------------------
struct FunctionPlotWidget<'a> {
    plot:       &'a FunctionPlot,
    population: Option<&'a Population>,
}

impl<'a> FunctionPlotWidget<'a> {
    fn new(plot: &'a FunctionPlot, population: Option<&'a Population>) -> Self {
        Self { plot, population }
    }
}

impl<'a> Widget for FunctionPlotWidget<'a> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let available = ui.available_size();
        let size = Vec2::new(available.x.max(2.0), available.y.max(2.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        if ui.is_rect_visible(rect) {
            let layout = PlotLayout::new(rect, self.plot.x_min, self.plot.x_max, 0.0, 1.0);
            let hover = response.hover_pos().and_then(|pos| {
                if !layout.plot_rect.contains(pos) { return None; }

                // pw: szerokość wykresu w pikselach
                let pw = layout.plot_rect.width() as f64;
                let hx = self.plot.x_min
                    + ((pos.x - layout.plot_rect.left()) as f64 / pw)
                    * (self.plot.x_max - self.plot.x_min);

                // hy: wartość funkcji w punkcie hx
                let hy = self.plot.eval(hx);
                if hy.is_finite() {
                    Some((hx, hy))
                } else {
                    None
                }
            });

            if response.hovered() {
                ui.ctx().request_repaint();
            }

            self.plot.paint(ui, ui.painter(), rect, hover, self.population);
        }

        response
    }
}

/// Stan współdzielony między wątkiem GUI a wątkiem GA.
/// Zamknięty w Arc<Mutex<>>, żeby oba wątki mogły go bezpiecznie czytać/pisać.
struct GaState {
    population: Population,
    /// Czy trwa aktualnie obliczanie nowej generacji?
    running: bool,
    /// Czy włączony jest tryb auto?
    /// Wątek pętlowy sprawdza tę flagę co sekundę.
    auto_active: bool,
    /// Czy wątek auto-calculate już działa?
    auto_thread_running: bool,
    /// Parametry GA edytowalne przez okno opcji.
    params: OptionsParams,
}

struct MyApp {
    plot: FunctionPlot,
    ga_state: Arc<Mutex<GaState>>,
    ctx: Option<egui::Context>,
    selected_idx: Option<usize>,
    /// Zmierzona szerokość paska przycisków z poprzedniej klatki.
    /// Używana do obliczenia lewego marginesu centrującego.
    btn_bar_width: f32,
    /// Stan okna opcji (widoczność + wartości robocze w trakcie edycji).
    options_window: OptionsWindow,
}

impl Default for MyApp {
    fn default() -> Self {
        let defaults = OptionsParams::default();
        let pop = Population::random(defaults.pop_size, -10.0, 10.0, FunctionPlot::target);
        let ga_state = Arc::new(Mutex::new(GaState {
            population: pop,
            running: false,
            auto_active: false,
            auto_thread_running: false,
            params: defaults.clone(),
        }));

        Self {
            plot: FunctionPlot::new(FunctionPlot::target, -10.0, 10.0),
            ga_state,
            ctx: None,
            selected_idx: None,
            btn_bar_width: 0.0,
            options_window: OptionsWindow::new(&defaults),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Zapamiętaj kontekst przy pierwszej klatce.
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }

        // Pobierz aktualny stan z mutexa (krótko, tylko żeby skopiować dane do wyświetlenia).
        let (population_snapshot, ga_running) = {
            let state = self.ga_state.lock().unwrap();
            (state.population.clone(), state.running)
        };

        egui::SidePanel::right("panel_populacja")
            .default_width(340.0)
            .resizable(true)
            .show(ctx, |ui| {
                // -- Prawa kolumna: podgląd populacji -----------------------
                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    let generation = population_snapshot.generation;
                    let best = population_snapshot.best()
                        .map(|c| format!("x={:.4}  f={:.4}", c.x, c.fitness))
                        .unwrap_or_default();

                    ui.label(
                        egui::RichText::new(format!("Pokolenie #{generation}"))
                            .strong()
                    );
                    ui.label(
                        egui::RichText::new(format!("Najlepszy: {best}"))
                            .strong()
                            .color(Color32::from_rgb(220, 140, 0))
                    );
                    ui.add_space(4.0);
                    ui.separator();

                    // Lista chromosomów – przewijalna, z możliwością zaznaczenia wiersza.
                    egui::ScrollArea::vertical()
                        .id_salt("pop_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (i, chrom) in population_snapshot.chromosomes.iter().enumerate() {
                                let text = format!("{:>2}. {}", i + 1, chrom.display_str());
                                let is_selected = self.selected_idx == Some(i);
                                let color = if i == 0 {
                                    Color32::from_rgb(220, 140, 0)   // najlepszy – pomarańczowy
                                } else {
                                    Color32::from_rgb(60, 120, 200) // reszta – niebieski
                                };
                                let label = egui::Button::selectable(
                                    is_selected,
                                    egui::RichText::new(text).monospace().color(color).size(11.0),
                                ).frame(false);
                                if ui.add(label).clicked() {
                                    self.selected_idx = if is_selected { None } else { Some(i) };
                                }
                            }
                        });
                });
            });

        // Sprawdź skróty klawiszowe (niezależnie od fokusa przycisku).
        let hotkey_calc  = ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::C));
        // Alt+R jest obsługiwany przez okno opcji, gdy jest otwarte – nie konsumuj go tutaj.
        let hotkey_reset = !self.options_window.open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::R));
        let hotkey_auto  = ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::A));

        // Odczytaj flagę auto z mutexa (potrzebna do wyświetlenia stanu przycisku).
        let auto_active = self.ga_state.lock().unwrap().auto_active;

        egui::CentralPanel::default().show(ctx, |ui| {
            // -- Środek: wykres + przyciski ----------------------------------
            // Zarezerwuj pasek na przyciski na dole, reszta idzie na wykres.
            let btn_height = 28.0;
            let spacing   = 6.0;
            let plot_height = (ui.available_height() - btn_height - spacing).max(2.0);

            // Wykres zajmuje górną część.
            let plot_size = Vec2::new(ui.available_width(), plot_height);
            ui.add_sized(plot_size, FunctionPlotWidget::new(&self.plot, Some(&population_snapshot)));

            ui.add_space(spacing);

            // -- Pasek przycisków wyśrodkowany – technika dwuklatkowa -------
            //
            // Egui rysuje UI lewa -> prawa, więc nie znamy szerokości paska
            // zanim go narysujemy, dlatego używamy wartości z poprzedniej
            // klatki (btn_bar_width), by obliczyć lewy margines.
            //
            //   lewy_margines = (szerokość_panelu − szerokość_paska) / 2
            //
            // Klatka 0: btn_bar_width == 0 → margines = panel/2 → pasek od środka
            //           (lekko za daleko, ale natychmiast po wyrenderowaniu
            //            zapisujemy prawdziwą szerokość)
            // Klatka 1+: btn_bar_width = zmierzona wartość → idealne wyśrodkowanie
            //
            // Uwaga: add_space działa w osi głównej bieżącego layoutu (top_down),
            // więc przesuwa tylko kursor pionowy. Żeby przesunąć poziomo,
            // wchodzimy w horizontal() i tam dodajemy margines.
            let available_width = ui.available_width();
            let left_margin = ((available_width - self.btn_bar_width) * 0.5).max(0.0);

            let row = ui.horizontal(|ui| {
                // Lewy margines wyrównujący pasek do centrum.
                ui.add_space(left_margin);

                let manual_enabled = !ga_running && !auto_active;

                let btn_calc = ui.add_enabled(
                    manual_enabled,
                    egui::Button::new("Następna generacja").shortcut_text("Alt+C"),
                );

                if btn_calc.clicked() || (manual_enabled && hotkey_calc) {
                    self.spawn_ga_step();
                }

                let btn_reset = ui.add_enabled(
                    manual_enabled,
                    egui::Button::new("Reset").shortcut_text("Alt+R"),
                );

                if btn_reset.clicked() || (manual_enabled && hotkey_reset) {
                    let pop_size = self.ga_state.lock().unwrap().params.pop_size;
                    let pop = Population::random(pop_size, -10.0, 10.0, FunctionPlot::target);
                    let mut state = self.ga_state.lock().unwrap();
                    state.population = pop;
                    self.selected_idx = None;
                }

                // Przycisk Auto – toggle, zmienia kolor gdy aktywny.
                let auto_label = if auto_active { "⏹ Auto" } else { "▶ Auto" };
                let auto_color = if auto_active {
                    Color32::from_rgb(220, 60, 60)  // Czerwony - lepiej widoczny w obu motywach
                } else {
                    ui.visuals().widgets.inactive.fg_stroke.color
                };

                let btn_auto = ui.add(
                    egui::Button::new(
                        egui::RichText::new(auto_label).color(auto_color)
                    ).shortcut_text("Alt+A"),
                );

                if btn_auto.clicked() || hotkey_auto {
                    let mut state = self.ga_state.lock().unwrap();
                    state.auto_active = !state.auto_active;
                    // Blokada: uruchamiaj wątek tylko jeśli nie działa
                    if state.auto_active && !state.auto_thread_running {
                        state.auto_thread_running = true;
                        drop(state);
                        Self::spawn_auto_thread(
                            Arc::clone(&self.ga_state),
                            self.ctx.clone(),
                        );
                    } else if !state.auto_active {
                        // Wyłączamy auto, wątek sam wyzeruje flagę po zakończeniu
                    }
                }

                ui.add_space(18.0);
                let btn_opcje = ui.add(egui::Button::new("Opcje").shortcut_text("Alt+O"));
                if btn_opcje.clicked() || ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.alt) {
                    let params = self.ga_state.lock().unwrap().params.clone();
                    self.options_window.open_with(&params);
                }
            });

            // Oblicz rzeczywistą szerokość samych przycisków (bez lewego marginesu).
            // row.response.rect obejmuje cały horizontal(), czyli left_margin + przyciski.
            // Odejmujemy left_margin, żeby w następnej klatce liczyć tylko przyciski.
            let measured = row.response.rect.width() - left_margin;
            if measured > 0.0 {
                self.btn_bar_width = measured;
            }
        });

        // Okno opcji – delegujemy całą logikę do OptionsWindow::show()
        if let Some(params) = self.options_window.show(ctx) {
            self.ga_state.lock().unwrap().params = params;
        }
    }
}

impl MyApp {
    /// Uruchamia długo żyjący wątek obsługujący auto-calculate.
    ///
    /// Wątek działa w nieskończoność (aż do zamknięcia programu). Co sekundę
    /// sprawdza flagę `auto_active` w mutexie:
    ///   - jeśli true  → odpala krok GA (jeśli poprzedni już się skończył)
    ///   - jeśli false → śpi dalej bez nic nie robiąc
    ///
    /// Dzięki temu GUI nie musi nic pollować – wystarczy ustawić flagę.
    fn spawn_auto_thread(state_arc: Arc<Mutex<GaState>>, ctx: Option<egui::Context>) {
        std::thread::spawn(move || {
            loop {
                let should_run = {
                    let state = state_arc.lock().unwrap();
                    state.auto_active && !state.running
                };

                if should_run {
                    Self::calculate(Arc::clone(&state_arc), &ctx);
                }

                // Jeśli auto zostało wyłączone, kończymy wątek.
                let still_active = state_arc.lock().unwrap().auto_active;
                if !still_active {
                    // Wyzeruj flagę auto_thread_running po zakończeniu wątku
                    let mut state = state_arc.lock().unwrap();
                    state.auto_thread_running = false;
                    break;
                }

                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
    }

    /// Odpala obliczenie nowej generacji w osobnym wątku.
    fn spawn_ga_step(&self) {
        let state_arc = Arc::clone(&self.ga_state);
        let ctx       = self.ctx.clone();

        // Oznacz jako "w trakcie obliczeń".
        {
            let mut state = state_arc.lock().unwrap();
            state.running = true;
        }

        std::thread::spawn(move || {
            Self::calculate(state_arc, &ctx);
        });
    }

    fn calculate(state_arc: Arc<Mutex<GaState>>, ctx: &Option<egui::Context>) {
        // Pobierz aktualną populację, numer pokolenia i aktualne parametry GA.
        let (old_pop, new_gen, pop_size, tournament_k, crossover_prob, mutation_prob) = {
            let state = state_arc.lock().unwrap();
            (
                state.population.clone(),
                state.population.generation + 1,
                state.params.pop_size,
                state.params.tournament_k,
                state.params.crossover_prob,
                state.params.mutation_prob,
            )
        };

        // Seed oparty na czasie, żeby każde pokolenie było naprawdę losowe.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(new_gen as u64)
            .wrapping_mul(new_gen as u64 + 1)
            .wrapping_add(0xdeadbeef);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let parents = &old_pop.chromosomes;

        // -- Selekcja turniejowa ---------------------------------------------
        // Losujemy K osobników, wygrywa ten z najwyższym fitness.
        // Wyobraź sobie turniej: losowo wybierasz K zawodników ze starej
        // populacji i przepuszczasz najlepszego dalej. Powtarzasz tyle razy,
        // ile potrzebujesz rodziców.
        let tournament = |rng: &mut rand::rngs::StdRng| -> &Chromosome {
            let mut best_idx = rng.gen_range(0..parents.len());
            for _ in 1..tournament_k {
                let idx = rng.gen_range(0..parents.len());
                if parents[idx].fitness > parents[best_idx].fitness {
                    best_idx = idx;
                }
            }
            &parents[best_idx]
        };

        // -- Krzyżowanie jednopunktowe ---------------------------------------
        // Wybieramy losowy punkt cięcia i sklejamy lewy kawałek jednego
        // rodzica z prawym kawałkiem drugiego.
        // Np. rodzic A: 1101|0011  rodzic B: 0010|1100
        //     dziecko:  1101|1100
        let crossover = |a: &Chromosome, b: &Chromosome, rng: &mut rand::rngs::StdRng| -> [bool; BITS] {
            let mut genes = a.genes;
            if rng.gen_bool(crossover_prob) {
                // punkt cięcia: 1..BITS-1
                let point = rng.gen_range(1..BITS);
                for i in point..BITS {
                    genes[i] = b.genes[i];
                }
            }
            genes
        };

        // -- Mutacja bitowa --------------------------------------------------
        // Każdy bit może się losowo odwrócić z prawdopodobieństwem MUTATION_PROB.
        // Wyobraź sobie kosmiczne promieniowanie, które z rzadka przełącza
        // jeden bit w DNA.
        let mutate = |genes: &mut [bool; BITS], rng: &mut rand::rngs::StdRng| {
            for bit in genes.iter_mut() {
                if rng.gen_bool(mutation_prob) {
                    *bit = !*bit;
                }
            }
        };

        // -- Elityzm: najlepszy osobnik przechodzi bez zmian -----------------
        let mut new_chromosomes: Vec<Chromosome> = Vec::with_capacity(pop_size);
        if let Some(elite) = old_pop.best() {
            new_chromosomes.push(elite.clone());
        }

        // -- Wypełnij resztę populacji dziećmi -------------------------------
        while new_chromosomes.len() < pop_size {
            let parent_a = tournament(&mut rng);
            let parent_b = tournament(&mut rng);

            let mut genes = crossover(parent_a, parent_b, &mut rng);
            mutate(&mut genes, &mut rng);

            let x = Chromosome::decode(&genes, -10.0, 10.0);
            let mut child = Chromosome { genes, fitness: 0.0, x };
            child.evaluate(FunctionPlot::target);
            new_chromosomes.push(child);
        }

        // Sortuj malejąco po fitness – najlepszy na górze.
        new_chromosomes.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap());

        let new_population = Population { chromosomes: new_chromosomes, generation: new_gen };

        // Zapisz wynik i zdejmij flagę "running".
        {
            let mut state = state_arc.lock().unwrap();
            state.population = new_population;
            state.running = false;
        }

        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 580.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "GeneticTool - Demo",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    ).unwrap();
}
