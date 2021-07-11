use crate::integrate::BlockData;
use crate::integrate::LogLevel;

use super::App;
use tui::{
    backend::Backend,
    layout::{Constraint, Corner, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Span, Spans},
    widgets::{
        Block, Borders, Cell, Chart, Dataset, Gauge, LineGauge, List, ListItem,
        Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame,
};

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {

    let page_components = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(20), Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(f.size());

    let devnet_status_components = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(78)].as_ref())
        .split(page_components[0]);

    let top_right_components = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(1)].as_ref())
        .split(devnet_status_components[1]);

    draw_devnet_status(f, app, devnet_status_components[0]);
    draw_services_status(f, app, top_right_components[0]);
    draw_mempool(f, app, top_right_components[1]);
    draw_blocks(f, app, page_components[1]);
    draw_help(f, app, page_components[2]);
}

fn draw_services_status<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let normal_style = Style::default().bg(Color::DarkGray);
    let header_cells = ["", "Service", "URL"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Gray)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(0);

    let services = vec![
        vec!["🟩", "stacks-node", "http://localhost:20443"],
        vec!["🟨", "stacks-api", "http://localhost:20443"],
        vec!["🟩", "stacks-explorer", "http://localhost:20443"],
        vec!["🟩", "bitcoind", "http://localhost:20443"],
    ];

    let rows = services.iter().map(|item| {
        let cells = item.iter().map(|c| Cell::from(*c));
        Row::new(cells).height(1).bottom_margin(0)
    });
    let t = Table::new(rows)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Services"))
        .widths(&[
            Constraint::Length(3),
            Constraint::Length(20),
            Constraint::Length(37),
        ]);
    f.render_widget(t, area);
}

fn draw_mempool<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let services = vec![
        vec!["00:00:00", "0xd08ed0fd32027fc686e7fc2b5f7474e5dbfc2ef102bba364be30c16c3ea4b860"],
    ];

    let rows = services.iter().map(|item| {
        let cells = item.iter().map(|c| Cell::from(*c));
        Row::new(cells).height(1).bottom_margin(0)
    });
    let t = Table::new(rows)
        .block(Block::default().borders(Borders::ALL).title("Mempool"))
        .widths(&[
            Constraint::Length(8),
            Constraint::Min(1),
        ]);
    f.render_widget(t, area);
}

fn draw_devnet_status<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    // let page_components = Layout::default()
    //     .direction(Direction::Vertical)
    //     .constraints([Constraint::Length(20), Constraint::Min(1), Constraint::Length(5)].as_ref())
    //     .split(f.size());

    let logs: Vec<ListItem> = app
        .logs
        .items
        .iter()
        .rev()
        .map(|log| {
            // Log level
            let (style, label) = match log.level {
                LogLevel::Error => (Style::default().fg(Color::LightRed), "ERRO"),
                LogLevel::Warning => (Style::default().fg(Color::LightYellow), "WARN"),
                LogLevel::Info => (Style::default().fg(Color::LightBlue), "INFO"),
                LogLevel::Success => (Style::default().fg(Color::LightGreen), "INFO"),
            };

            // let header = Spans::from(vec![
            //     Span::styled(format!("{:<9}", level), s),
            //     Span::raw(" "),
            //     Span::styled(
            //         "2020-01-01 10:00:00",
            //         Style::default().fg(Color::DarkGray),
            //     ),
            // ]);
            // The event gets its own line
            let log = Spans::from(vec![
                Span::styled(format!("{:<9}", label), style),
                Span::raw(" "),
                Span::raw(log.message.clone())]);

            ListItem::new(vec![log])
        })
        .collect();
    let logs_component = List::new(logs)
        .block(Block::default().borders(Borders::ALL).title("Stacks Devnet"))
        .start_corner(Corner::BottomLeft);
    f.render_widget(logs_component, area);
}

fn draw_blocks<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let t = Table::new(vec![])
        .block(Block::default().borders(Borders::ALL))
        .widths(&[]);
    f.render_widget(t, area);

    let blocks_components = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)].as_ref())
        .split(area);

    let titles = app.tabs.titles.iter().map(|s| s.clone()).collect();
    let blocks = Tabs::new(titles)
        .divider("")
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .highlight_style(Style::default().bg(Color::White).fg(Color::Black))
        .block(Block::default().borders(Borders::NONE))
        .select(app.tabs.index);

    let block_details_components = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(2)
        .vertical_margin(1)
        .constraints([Constraint::Length(75), Constraint::Min(1)].as_ref())
        .split(blocks_components[1]);

    f.render_widget(blocks, blocks_components[0]);

    if app.tabs.titles.is_empty() {
        return;
    }
    let selected_block = &app.blocks[(app.tabs.titles.len() - 1) - app.tabs.index].clone();

    draw_block_details(f, app, block_details_components[0], &selected_block);
    draw_transactions(f, app, block_details_components[1], &selected_block);
}

fn draw_block_details<B>(f: &mut Frame<B>, app: &mut App, area: Rect, block: &BlockData)
where
    B: Backend,
{
    let paragraph = Paragraph::new(String::new())
        .block(Block::default().borders(Borders::RIGHT).title("Block Informations"));
    f.render_widget(paragraph, area);

    let labels = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
        ].as_ref())
        .split(area);

    let label = "Block height:".to_string();
    let paragraph = Paragraph::new(label)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[1]);

    let value = format!("{}", block.block_height);
    let paragraph = Paragraph::new(value)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[2]);

    let label = "Block hash:".to_string();
    let paragraph = Paragraph::new(label)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[3]);

    let value = format!("{}", block.block_hash);
    let paragraph = Paragraph::new(value)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[4]);

    let label = "Bitcoin block height:".to_string();
    let paragraph = Paragraph::new(label)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[5]);

    let value = format!("{}", block.bitcoin_block_height);
    let paragraph = Paragraph::new(value)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[6]);

    let label = "Bitcoin block hash:".to_string();
    let paragraph = Paragraph::new(label)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[7]);

    let value = format!("{}", block.bitcoin_block_hash);
    let paragraph = Paragraph::new(value)
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(paragraph, labels[8]);

    // TODO(ludo): PoX informations
    // TODO(ludo): Mining informations (miner, VRF)
}

fn draw_transactions<B>(f: &mut Frame<B>, app: &mut App, area: Rect, block: &BlockData)
where
    B: Backend,
{
    let normal_style = Style::default().bg(Color::DarkGray);
    let header_cells = ["", "Txid", "Result"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Gray)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(0);

    let transactions: Vec<ListItem> = block.transactions.iter().map(|t| {
            let tx_info = Spans::from(vec![
                Span::styled(
                    match t.success {
                        true => "🟩",
                        false => "🟥"
                    },
                    Style::default(),
                ),
                Span::raw(" "),
                Span::styled(
                    t.txid.clone(),
                    Style::default(),
                ),
                Span::raw(" "),
                Span::styled(
                    t.result.clone(),
                    Style::default(),
                ),
            ]);
            ListItem::new(vec![
                tx_info,
                // events,
            ])
        })
        .collect();

    let list = List::new(transactions)
        .block(Block::default().borders(Borders::NONE).title("Transactions"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        // .start_corner(Corner::BottomLeft);
        .highlight_symbol("* ");

    f.render_widget(list, area);

}

fn draw_help<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let help = " ⬅️  ➡️  Explore blocks          ⬆️  ⬇️  Explore transactions          0️⃣  Genesis Reset";
    let paragraph = Paragraph::new(help.clone())
        .style(Style::default().bg(Color::Black).fg(Color::White))
        .block(Block::default().borders(Borders::NONE));

    f.render_widget(paragraph, area);
}