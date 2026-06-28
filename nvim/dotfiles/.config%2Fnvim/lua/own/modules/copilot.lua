vim.cmd('packadd copilot.lua')

require('copilot').setup({
    suggestion = {
        enabled = true,
        auto_trigger = true,
        debounce = 100,
        keymap = {
            accept = "<C-l>",
            next = "<C-j>",
            prev = "<C-k>",
        },
    },
})

vim.keymap.set('n', vim.g.mapleader..'?', '<cmd>Copilot panel<cr>')
