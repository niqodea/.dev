local M = {}

function M.yank_path()
    local path = vim.fn.expand('%:.')
    vim.fn.setreg('+', path)
end

-- Reloads the buffers without edits whose files changed on disk, and lists in quickfix the buffers with edits
function M.reload_buffers()
    local qf_list = {}
    for _, buf in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(buf) and vim.bo[buf].buftype == '' and vim.api.nvim_buf_get_name(buf) ~= '' then
            if vim.bo[buf].modified then
                -- Checking would take the file's new timestamp as seen, which silences nvim's warning on writing over it
                table.insert(qf_list, { bufnr = buf, lnum = 1, col = 1, text = 'buffer modified, not reloaded' })
            else
                -- 'autoread' reloads the buffer if its file changed, and :checktime alone skips hidden buffers
                vim.cmd.checktime(buf)
            end
        end
    end
    if #qf_list > 0 then
        vim.fn.setqflist(qf_list)
        vim.cmd('copen')
    end
end

function M.move_window(direction, split)
    local neighbor = vim.fn.win_getid(vim.fn.winnr(direction))
    if neighbor == vim.api.nvim_get_current_win() then return end
    vim.api.nvim_win_set_config(0, { win = neighbor, split = split })
end

return M
