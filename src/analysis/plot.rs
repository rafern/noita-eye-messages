use dataviz::figure::{
    canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig, datasets::bardataset::BarDataset, display::hover::Hover, drawers::drawer::Drawer, figuretypes::groupbarchart::GroupBarChart, utilities::orientation::Orientation
};
use fontconfig::Fontconfig;
use minifb::{Key, MouseMode, Window, WindowOptions};

use crate::utils::threading::AsyncTaskList;

use super::freq::{UnitFrequency, UnitTotals};

const BLACK: [u8; 3] = [0, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];
const GREY: [u8; 3] = [220, 220, 220];
const TARGET_FPS_LOW: usize = 20;
const TARGET_FPS_HIGH: usize = 60;
const MAX_WIDTH: u32 = 1400;
const MAX_HEIGHT: u32 = 400;
const MARGIN: u32 = 80;
const CELL_LEN: u32 = 12;
const HSV_SAT: f64 = 1.0;
const HSV_VAL: f64 = 0.9;
const INTERACTIVE: bool = false; // TODO enable once plot hovers are fixed in dataviz

pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let x = 1.0 - ((h * 6.0).rem_euclid(2.0) - 1.0).abs();
    let m = v - c;

    let temp = {
        if h < 0.16666666666666666 { [c, x, 0.0] }
        else if h < 0.3333333333333333 { [x, c, 0.0] }
        else if h < 0.5 { [0.0, c, x] }
        else if h < 0.6666666666666666 { [0.0, x, c] }
        else if h < 0.8333333333333333 { [x, 0.0, c] }
        else { [c, 0.0, x] }
    };

    [
        ((temp[0] + m) * 255.0) as u8,
        ((temp[1] + m) * 255.0) as u8,
        ((temp[2] + m) * 255.0) as u8,
    ]
}

pub fn get_default_font_path() -> String {
    String::from(Fontconfig::new().unwrap().find("sans-serif", None).unwrap().path.to_str().unwrap())
}

pub fn get_default_figure_config() -> FigureConfig {
    let font = get_default_font_path();

    FigureConfig {
        font_size_title: 20.0,
        font_size_label: 16.0,
        font_size_legend: 14.0,
        font_size_axis: 10.0,
        color_axis: BLACK,
        color_background: WHITE,
        color_grid: GREY,
        color_title: BLACK,
        num_axis_ticks: 10, // XXX slightly wrong, it's actually ticks + 1, but it works out in our favour
        num_grid_horizontal: CELL_LEN as usize, // XXX misnomer, it's actually length in pixels
        num_grid_vertical: CELL_LEN as usize, // XXX misnomer, it's actually length in pixels
        font_label: Some(font.clone()),
        font_title: Some(font),
    }
}

pub fn plot<T: Drawer + Hover + Send>(task_list: &mut AsyncTaskList, mut plot: T, title: &str, target_width: u32, target_height: u32) {
    let title_copy = String::from(title);

    task_list.add_async_or_sync(move || {
        let width = target_width + MARGIN * 2;
        let height = target_height + MARGIN * 2;
        let mut canvas = PixelCanvas::new(width, height, WHITE, MARGIN);
        plot.draw(&mut canvas);

        // XXX modified version of Winop::display_interactive, with inline
        //     Winop::canvas_to_buffer
        let mut window = Window::new(
            format!("noita-eye-messages - {}", title_copy).as_str(),
            width as usize,
            height as usize,
            WindowOptions::default(),
        ).unwrap_or_else(|e| panic!("Unable to open Window: {e}"));

        let mut buffer: Vec<u32> = canvas
            .buffer
            .chunks_exact(3)
            .map(|rgb| {
                let r = rgb[0] as u32;
                let g = rgb[1] as u32;
                let b = rgb[2] as u32;
                0xFF000000 | (r << 16) | (g << 8) | b
            })
            .collect();

        if INTERACTIVE {
            let mut last_mouse_pos: Option<(f32, f32)> = None;
            window.set_target_fps(TARGET_FPS_HIGH);

            while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
                let mut mouse_pos_wrapped = window.get_mouse_pos(MouseMode::Pass);
                if let Some(mouse_pos) = mouse_pos_wrapped {
                    // HACK workaround for default mouse pos
                    if mouse_pos.0 == 0f32 && mouse_pos.1 == 0f32 { mouse_pos_wrapped = None }
                }

                if mouse_pos_wrapped != last_mouse_pos {
                    last_mouse_pos = mouse_pos_wrapped;

                    if let Some(mouse_pos) = mouse_pos_wrapped {
                        let (mouse_x, mouse_y) = (mouse_pos.0 as u32, mouse_pos.1 as u32);

                        if let Some(updated_buffer) = plot.handle_hover(mouse_x, mouse_y, &canvas) {
                            buffer = updated_buffer;
                        }
                    }
                }

                window.update_with_buffer(&buffer, width as usize, height as usize).unwrap();
            }
        } else {
            window.set_target_fps(TARGET_FPS_LOW);
            while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
                window.update_with_buffer(&buffer, width as usize, height as usize).unwrap();
            }
        }
    });
}

pub fn bar_chart(task_list: &mut AsyncTaskList, title: &str, x_label: &str, y_label: &str, totals: &UnitTotals) {
    let mut dataset = BarDataset::new("Data", hsv_to_rgb(0.0, HSV_SAT, HSV_VAL));
    let mut x_min = usize::MAX;
    let mut x_max = 0;
    let mut y_max = 0;

    for i in 0..totals.data.len() {
        let v = totals.data[i];
        if v != 0 {
            dataset.add_data(i as f64, v as f64);
            x_min = x_min.min(i);
            x_max = x_max.max(i);
            y_max = y_max.max(v);
        }
    }

    let mut cfg = get_default_figure_config();
    cfg.num_axis_ticks = y_max;

    let mut histogram = GroupBarChart::new(title, x_label, y_label, Orientation::Vertical, cfg);
    histogram.add_dataset(dataset);
    plot(task_list, histogram, title, (CELL_LEN * (x_max - x_min + 1) as u32).min(MAX_WIDTH), (CELL_LEN * (y_max + 1) as u32).min(MAX_HEIGHT));
}

pub fn freq_bar_chart(task_list: &mut AsyncTaskList, title: &str, x_label: &str, y_label: &str, freqs: Vec<UnitFrequency>) {
    let mut histogram = GroupBarChart::new(title, x_label, y_label, Orientation::Vertical, get_default_figure_config());
    let mut x_max = 0;

    let mut f = 0;
    let f_total = freqs.len();
    for freq in freqs {
        let hue: f64 = if f_total == 2 && f == 1 {
            // special case for 2 frequency distributions so your eyes don't bleed
            0.666666
        } else {
            f as f64 / f_total as f64
        };

        let mut dataset = BarDataset::new(freq.name.as_str(), hsv_to_rgb(hue, HSV_SAT, HSV_VAL));

        for i in 0..freq.data.len() {
            let v = freq.data[i];
            if v > 0.0 {
                dataset.add_data(i as f64, v as f64);
                x_max = x_max.max(i);
            } else {
                break; // it's sorted, so everything else is 0
            }
        }

        histogram.add_dataset(dataset);
        f += 1;
    }

    plot(task_list, histogram, title, (CELL_LEN * (x_max + 1) as u32).min(MAX_WIDTH), MAX_HEIGHT);
}