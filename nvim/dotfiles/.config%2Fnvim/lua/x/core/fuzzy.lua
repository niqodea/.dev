local M = {}

vim.cmd('packadd fzf-lua')
M.fzf_lua = require('fzf-lua')

M.fzf_lua.setup{
    -- Set POSIX-compliant find command for MacOS
    files = { cmd = 'find . -type f' }
}

-- Fuzzy find through files
vim.keymap.set('n', '<leader>f-', function() M.fzf_lua.files() end)
vim.keymap.set('n', '<leader>f/', function() M.fzf_lua.live_grep() end)
vim.keymap.set('n', '<leader>f*', function() M.fzf_lua.grep_cword() end)
vim.keymap.set('v', '<leader>f*', function() M.fzf_lua.grep_visual() end)

-- Fuzzy find through files in the current directory
vim.keymap.set('n', '<leader>F-', function() M.fzf_lua.files({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('n', '<leader>F/', function() M.fzf_lua.live_grep({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('n', '<leader>F*', function() M.fzf_lua.grep_cword({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('v', '<leader>F*', function() M.fzf_lua.grep_visual({ cwd = vim.fn.expand('%:p:h') }) end)

return M
