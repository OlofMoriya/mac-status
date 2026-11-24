use crate::Status;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Application, Bounds, Context, InteractiveElement, IntoElement, ParentElement,
    Render, StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};

struct Dashboard(Vec<Status>);

impl Render for Dashboard {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x141820))
            .font_family("Helvetica Neue")
            .text_color(rgb(0xe8ecf4))
            .child(
                div()
                    .p_4()
                    .text_xl()
                    .child(format!("Mac Status · {} checks", self.0.len())),
            )
            .child(
                div()
                    .id("checks")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_4()
                    .pb_4()
                    .children(self.0.iter().enumerate().map(|(index, row)| {
                        let heading = index == 0 || self.0[index - 1].section != row.section;
                        let color = match row.state {
                            "present" | "configured" | "clean" | "recent" => 0x86dbab,
                            "missing" | "wrong target" | "stale" => 0xff9999,
                            _ => 0xe9c687,
                        };
                        div()
                            .when(heading, |el| {
                                el.child(div().mt_3().mb_2().text_lg().child(row.section))
                            })
                            .child(
                                div()
                                    .mb_2()
                                    .p_3()
                                    .rounded_md()
                                    .bg(rgb(0x202633))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .gap_2()
                                            .child(row.name.clone())
                                            .child(div().text_color(rgb(color)).child(row.state)),
                                    )
                                    .child(
                                        div()
                                            .mt_1()
                                            .text_sm()
                                            .text_color(rgb(0xaab6cc))
                                            .child(row.detail.clone()),
                                    ),
                            )
                    })),
            )
    }
}

pub fn show(rows: Vec<Status>) {
    Application::new().run(move |cx: &mut App| {
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let rows = if std::env::var_os("MAC_STATUS_GUI_PROBE").is_some() {
            vec![Status {
                repo_path: None,
                section: "GPUI text rendering probe",
                name: "If you can read this, text renders".into(),
                state: "present",
                detail: "This window uses a single synthetic status row.".into(),
            }]
        } else {
            rows
        };
        let bounds = Bounds::centered(None, size(px(780.), px(640.)), cx);
        match cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Mac Status".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Dashboard(rows)),
        ) {
            Ok(_) => cx.activate(true),
            Err(error) => {
                eprintln!("mac-status: {error}");
                cx.quit();
            }
        }
    });
}
