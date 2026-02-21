use eframe::egui;

// ---------------------------------------------------------------------------
// Parametry GA przechowywane po zatwierdzeniu przez użytkownika
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct OptionsParams {
    pub mutation_prob:  f64,
    pub crossover_prob: f64,
    pub tournament_k:   usize,
    pub pop_size:       usize,
}

impl Default for OptionsParams {
    fn default() -> Self {
        Self {
            mutation_prob:  0.05,
            crossover_prob: 0.8,
            tournament_k:   3,
            pop_size:       20,
        }
    }
}

// ---------------------------------------------------------------------------
// OptionsWindow – stan edytowalny, metoda show() rysuje okno
//
// Typowe użycie:
//   if let Some(params) = self.options_window.show(ctx) {
//       // użytkownik nacisnął OK – params zawiera zatwierdzone wartości
//   }
// ---------------------------------------------------------------------------

pub struct OptionsWindow {
    /// Czy okno jest aktualnie widoczne.
    pub open: bool,
    // Wartości robocze (edytowane przez użytkownika, ale jeszcze niezatwierdzone).
    mutation_prob:  f64,
    crossover_prob: f64,
    tournament_k:   usize,
    pop_size:       usize,
    /// Zmierzona szerokość paska przycisków z poprzedniej klatki (do centrowania).
    btn_bar_width:  f32,
}

impl OptionsWindow {
    /// Tworzy okno opcji z podanymi wartościami startowymi.
    pub fn new(params: &OptionsParams) -> Self {
        Self {
            open:           false,
            mutation_prob:  params.mutation_prob,
            crossover_prob: params.crossover_prob,
            tournament_k:   params.tournament_k,
            pop_size:       params.pop_size,
            btn_bar_width:  0.0,
        }
    }

    /// Otwiera okno i kopiuje do niego aktualne parametry do edycji.
    pub fn open_with(&mut self, params: &OptionsParams) {
        self.mutation_prob  = params.mutation_prob;
        self.crossover_prob = params.crossover_prob;
        self.tournament_k   = params.tournament_k;
        self.pop_size       = params.pop_size;
        self.btn_bar_width  = 0.0;
        self.open           = true;
    }

    /// Rysuje okno; zwraca `Some(params)` gdy użytkownik zatwierdził (OK / Enter),
    /// `None` gdy okno jest otwarte lub zostało anulowane.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<OptionsParams> {
        if !self.open {
            return None;
        }

        let mut confirmed = false;
        let mut cancelled = false;

        // .default_pos + .pivot: domyślnie wyśrodkowane, ale okno pozostaje
        // przeciągalne (w odróżnieniu od .anchor(), które przypina co klatkę).
        let center = ctx.screen_rect().center();
        egui::Window::new("Opcje")
            .collapsible(false)
            .resizable(false)
            .fixed_size([460.0, 170.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(center)
            .open(&mut self.open)
            .show(ctx, |ui| {
                egui::Grid::new("options_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Prawdopodobieństwo mutacji (MUTATION_PROB):");
                        ui.add(
                            egui::DragValue::new(&mut self.mutation_prob)
                                .speed(0.001)
                                .range(0.0..=1.0),
                        );
                        ui.end_row();

                        ui.label("Prawdopodobieństwo krzyżowania (CROSSOVER_PROB):");
                        ui.add(
                            egui::DragValue::new(&mut self.crossover_prob)
                                .speed(0.001)
                                .range(0.0..=1.0),
                        );
                        ui.end_row();

                        ui.label("Rozmiar turnieju (TOURNAMENT_K):");
                        ui.add(
                            egui::DragValue::new(&mut self.tournament_k)
                                .speed(0.1)
                                .range(1..=20),
                        );
                        ui.end_row();

                        ui.label("Rozmiar populacji (POP_SIZE):");
                        ui.add(
                            egui::DragValue::new(&mut self.pop_size)
                                .speed(0.1)
                                .range(2..=100),
                        );
                        ui.end_row();
                    });

                ui.add_space(12.0);
                let available_width = ui.available_width();

                // Klatka 0: btn_bar_width==0, lewy margines=0, wszystkie przyciski
                // renderują się od lewej i zostają zmierzone. Klatka 1+: idealne centrowanie.
                let left_margin = if self.btn_bar_width > 0.0 {
                    ((available_width - self.btn_bar_width) * 0.5).max(0.0)
                } else {
                    0.0
                };

                let btn_row = ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    if ui.add(egui::Button::new("OK").shortcut_text("Enter")).clicked() {
                        confirmed = true;
                    }
                    if ui.add(egui::Button::new("Anuluj").shortcut_text("Esc")).clicked() {
                        cancelled = true;
                    }
                    if ui.add(egui::Button::new("Reset").shortcut_text("Alt+R")).clicked()
                        || ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::R))
                    {
                        let d = OptionsParams::default();
                        self.mutation_prob  = d.mutation_prob;
                        self.crossover_prob = d.crossover_prob;
                        self.tournament_k   = d.tournament_k;
                        self.pop_size       = d.pop_size;
                        ctx.request_repaint();
                    }
                });

                let measured = btn_row.response.rect.width() - left_margin;
                if measured > 0.0 {
                    self.btn_bar_width = measured;
                }

                // Enter = OK, Escape = Anuluj
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
                    confirmed = true;
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                    cancelled = true;
                }
            });

        if confirmed {
            self.open = false;
            return Some(OptionsParams {
                mutation_prob:  self.mutation_prob,
                crossover_prob: self.crossover_prob,
                tournament_k:   self.tournament_k,
                pop_size:       self.pop_size,
            });
        }

        if cancelled {
            self.open = false;
        }

        None
    }
}
