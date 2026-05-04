//! List selection helpers shared by sender/receiver panels.

use crate::account_view::Panel;
use crate::models::AcctRow;

pub fn clamp_sel(sel: &mut usize, len: usize) {
    if len == 0 {
        *sel = 0;
    } else if *sel >= len {
        *sel = len - 1;
    }
}

pub fn receiver_table_len(receiver_rows: &[AcctRow]) -> usize {
    receiver_rows.len() + 1
}

pub fn move_selection_down(sel: &mut usize, len: usize) {
    if len > 0 {
        *sel = (*sel + 1).min(len - 1);
    }
}

pub fn move_selection_up(sel: &mut usize) {
    *sel = sel.saturating_sub(1);
}

pub fn selected_to_receiver(receiver_rows: &[AcctRow], recv_sel: usize) -> Option<&AcctRow> {
    if recv_sel == 0 {
        None
    } else {
        receiver_rows.get(recv_sel - 1)
    }
}

pub fn selected_row_for_panel<'a>(
    active: Panel,
    owner_rows: &'a [AcctRow],
    owner_sel: usize,
    receiver_rows: &'a [AcctRow],
    recv_sel: usize,
) -> Option<&'a AcctRow> {
    match active {
        Panel::Owner => owner_rows.get(owner_sel),
        Panel::Receivers => selected_to_receiver(receiver_rows, recv_sel).or(owner_rows.first()),
    }
}
