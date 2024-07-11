use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget},
};

use super::peers::Peer;

pub struct InCommingVerify<'a>(pub &'a Peer);

impl<'a> Widget for InCommingVerify<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);

        let peer = self.0;
        let hostname = peer
            .0
            .hostname
            .clone()
            .unwrap_or_else(|| String::from("unknown"));
        let text = format!("{}:({})", hostname, peer.0.peer_id);

        let block = Block::bordered().style(Style::default().bg(Color::Gray));
        let lines = vec![
            Line::from(vec![Span::raw(text)]).centered(),
            Line::default(),
            Line::from(vec![
                Span::styled("Y", Style::default().fg(Color::Red)),
                Span::raw("es / "),
                Span::styled("N", Style::default().fg(Color::Red)),
                Span::raw("o"),
            ])
            .centered(),
        ];

        Paragraph::new(lines).block(block).render(area, buf);
    }
}
