vim.cmd('packadd gitsigns.nvim')
require('gitsigns').setup()

-- One column for git signs, always shown so the text does not shift
vim.wo.signcolumn = 'yes:1'

vim.api.nvim_set_hl(0, "GitSignsAdd", { ctermfg = 'DarkGreen' })
vim.api.nvim_set_hl(0, "GitSignsChange", { ctermfg = 'DarkYellow' })
vim.api.nvim_set_hl(0, "GitSignsDelete", { ctermfg = 'DarkRed' })

-- TODO: Add gitsigns keybindings for hunks

local fzf_lua = require('x.core.fuzzy').fzf_lua

vim.keymap.set('n', '<leader>fg-', function()
    fzf_lua.git_files()
end)
vim.keymap.set('n', '<leader>fg/', function()
    fzf_lua.live_grep({ cmd = "git grep --line-number --column" })
end)
vim.keymap.set('n', '<leader>fg*', function()
    fzf_lua.grep_cword({ cmd = "git grep --line-number --column" })
end)
vim.keymap.set('v', '<leader>fg*', function()
    fzf_lua.grep_visual({ cmd = "git grep --line-number --column" })
end)

-- The capital F searches from the current file's folder
vim.keymap.set('n', '<leader>Fg-', function()
    fzf_lua.git_files({ cwd = vim.fn.expand('%:p:h') }) end)
vim.keymap.set('n', '<leader>Fg/', function()
    fzf_lua.live_grep({ cwd = vim.fn.expand('%:p:h'), cmd = "git grep --line-number --column" })
end)
vim.keymap.set('n', '<leader>Fg*', function()
    fzf_lua.grep_cword({ cwd = vim.fn.expand('%:p:h'), cmd = "git grep --line-number --column" })
end)
vim.keymap.set('v', '<leader>Fg*', function()
    fzf_lua.grep_visual({ cwd = vim.fn.expand('%:p:h'), cmd = "git grep --line-number --column" })
end)

require('x.modules.git-commands')
