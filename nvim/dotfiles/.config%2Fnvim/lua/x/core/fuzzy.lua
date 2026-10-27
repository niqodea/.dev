local M = {}

vim.cmd('packadd fzf-lua')
M.fzf_lua = require('fzf-lua')

M.fzf_lua.setup {
    -- A POSIX find, which works on macOS too
    files = { cmd = 'find . -type f' },
}

vim.keymap.set('n', '<leader>f-', function() M.fzf_lua.files() end)
vim.keymap.set('n', '<leader>f/', function() M.fzf_lua.live_grep() end)
vim.keymap.set('n', '<leader>f*', function() M.fzf_lua.grep_cword() end)
vim.keymap.set('v', '<leader>f*', function() M.fzf_lua.grep_visual() end)

-- The current buffer is left out, so the previous one comes first
vim.keymap.set('n', '<leader>fb', function() M.fzf_lua.buffers({ ignore_current_buffer = true }) end)

-- Recently opened files in the current folder, newest first, this session's too
vim.keymap.set(
    'n',
    '<leader>fr',
    function() M.fzf_lua.oldfiles({ cwd_only = true, include_current_session = true }) end
)

-- The capital F searches from the current file's folder
vim.keymap.set('n', '<leader>F-', function() M.fzf_lua.files({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('n', '<leader>F/', function() M.fzf_lua.live_grep({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('n', '<leader>F*', function() M.fzf_lua.grep_cword({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('v', '<leader>F*', function() M.fzf_lua.grep_visual({ cwd = vim.fn.expand('%:p:h') }) end)

return M
