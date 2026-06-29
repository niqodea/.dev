-- Setup language servers for each language
vim.cmd('packadd cmp-nvim-lsp')
local cmp_nvim_lsp = require('cmp_nvim_lsp')
cmp_nvim_lsp.setup()
local language_files = vim.api.nvim_get_runtime_file('lua/x/modules/lsp-languages/*.lua', true)
for _, language_file in ipairs(language_files) do
    local language = language_file:match(".+/([^/]+)%.lua")
    require('x.modules.lsp-languages.' .. language)(cmp_nvim_lsp)
end

-- Mnemonics: a staple symbol of many languages, also easy to type
local lsp_prefix = vim.g.mapleader..';'

vim.keymap.set('n', lsp_prefix..'d', vim.lsp.buf.definition)
vim.keymap.set('n', lsp_prefix..'D', vim.lsp.buf.declaration)
vim.keymap.set('n', lsp_prefix..'e', vim.diagnostic.open_float)
vim.keymap.set('n', lsp_prefix..'i', vim.lsp.buf.implementation)
vim.keymap.set('n', lsp_prefix..'h', vim.lsp.buf.hover)
vim.keymap.set('n', lsp_prefix..'n', vim.lsp.buf.rename)
vim.keymap.set('n', lsp_prefix..'r', vim.lsp.buf.references)
vim.keymap.set('n', lsp_prefix..'t', vim.lsp.buf.type_definition)

-- Disable inline and gutter diagnostic
-- Ref: https://github.com/neovim/nvim-lspconfig/issues/662#issuecomment-983423169
vim.diagnostic.config({virtual_text = false, signs = false})

-- Use meaningful colors for errors and warnings
vim.api.nvim_set_hl(0, "DiagnosticUnderlineError", { underline = true, ctermfg = 'DarkRed' })
vim.api.nvim_set_hl(0, "DiagnosticUnderlineWarn", { underline = true, ctermfg = 'DarkYellow' })

vim.o.omnifunc = 'v:lua.vim.lsp.omnifunc'
vim.cmd('packadd nvim-cmp')
local cmp = require('cmp')
cmp.setup {
    sources = { { name = 'nvim_lsp' } },
    mapping = cmp.mapping.preset.insert(
        {
            -- Scroll through completion item docs
            ['<C-u>'] = cmp.mapping.scroll_docs(-4),
            ['<C-d>'] = cmp.mapping.scroll_docs(4),
            -- Confirm selection item
            ['<CR>'] = cmp.mapping.confirm {
                behavior = cmp.ConfirmBehavior.Replace,
                select = false,  -- Only select when explicitly chosen
            },
        }
    ),
    snippet = {
        expand = function(args)
            require('x.core').snippet.snippy.expand_snippet(args.body)
        end,
    },
}

vim.cmd('packadd aerial.nvim')
require('aerial').setup({
    backends = { 'lsp' },
    -- We need to tell aerial we disabled LSP diagnostics to update on buffer change instead
    -- Ref: https://github.com/stevearc/aerial.nvim/issues/54#issuecomment-1028611526
    lsp = {
        diagnostics_trigger_update = false,
    },
    layout = {
        max_width = 0.9,
        min_width = 0.9,
    },
    float = {
        relative = 'win',
        max_height = 0.9,
        min_height = 0.9,
    }
})
vim.keymap.set('n', lsp_prefix..'s', '<cmd>AerialOpen float<CR>')
