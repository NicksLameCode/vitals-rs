use gtk4 as gtk;
use gtk::prelude::*;

use vitals_core::history::TimePoint;

const GRAPH_WIDTH: i32 = 208;
const GRAPH_HEIGHT: i32 = 90;

/// A Cairo-drawn bar chart for sensor history visualization.
pub struct HistoryGraphWidget {
    pub drawing_area: gtk::DrawingArea,
}

impl HistoryGraphWidget {
    pub fn new() -> Self {
        let drawing_area = gtk::DrawingArea::new();
        drawing_area.set_size_request(GRAPH_WIDTH, GRAPH_HEIGHT);
        drawing_area.set_content_width(GRAPH_WIDTH);
        drawing_area.set_content_height(GRAPH_HEIGHT);

        Self { drawing_area }
    }

    /// Set the data to display and trigger a redraw.
    pub fn set_data(&self, samples: Vec<TimePoint>, label: String) {
        self.drawing_area.set_draw_func(move |_area, cr, width, height| {
            let w = width as f64;
            let h = height as f64;

            // Background
            cr.set_source_rgb(0.15, 0.15, 0.15);
            let _ = cr.paint();

            if samples.is_empty() {
                return;
            }

            // Find min/max values
            let values: Vec<f64> = samples.iter().filter_map(|p| p.v).collect();
            if values.is_empty() {
                return;
            }

            let v_min = values.iter().cloned().fold(f64::INFINITY, f64::min);
            let v_max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let range = if (v_max - v_min).abs() < f64::EPSILON {
                1.0
            } else {
                v_max - v_min
            };

            // Draw bars
            let bar_width = w / samples.len() as f64;
            cr.set_source_rgba(0.3, 0.6, 1.0, 0.8);

            for (i, point) in samples.iter().enumerate() {
                if let Some(v) = point.v {
                    let normalized = (v - v_min) / range;
                    let bar_height = normalized * (h - 20.0);
                    let x = i as f64 * bar_width;
                    let y = h - 10.0 - bar_height;

                    cr.rectangle(x, y, bar_width.max(1.0), bar_height);
                    let _ = cr.fill();
                }
            }

            // Draw label
            cr.set_source_rgb(0.9, 0.9, 0.9);
            cr.set_font_size(10.0);
            let _ = cr.move_to(4.0, 12.0);
            let _ = cr.show_text(&label);

            // Draw Y-axis labels
            cr.set_font_size(8.0);
            let _ = cr.move_to(4.0, h - 2.0);
            let _ = cr.show_text(&format!("{v_min:.1}"));
            let _ = cr.move_to(4.0, 22.0);
            let _ = cr.show_text(&format!("{v_max:.1}"));
        });

        self.drawing_area.queue_draw();
    }
}
