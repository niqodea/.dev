-- Servers work on the folder nvim runs in, like the git keys, and leave files outside it alone
-- A file counts where it really lives, so a symlink counts for its target, and a file not saved yet for its path
vim.lsp.config('*', {
    root_dir = function(buf, on_dir)
        local cwd = vim.fn.getcwd()
        local path = vim.uv.fs_realpath(vim.api.nvim_buf_get_name(buf)) or vim.api.nvim_buf_get_name(buf)
        if vim.fs.relpath(cwd, path) then on_dir(cwd) end
    end,
})

local language_files = vim.api.nvim_get_runtime_file('lua/x/modules/lsp-languages/*.lua', true)
for _, language_file in ipairs(language_files) do
    local language = language_file:match('.+/([^/]+)%.lua')
    require('x.modules.lsp-languages.' .. language)
end

-- ; ends statements in many languages, and is easy to type
local lsp_prefix = '<leader>;'

vim.keymap.set('n', lsp_prefix .. 'd', vim.lsp.buf.definition)
vim.keymap.set('n', lsp_prefix .. 'D', vim.lsp.buf.declaration)
vim.keymap.set('n', lsp_prefix .. 'e', vim.diagnostic.open_float)
vim.keymap.set('n', lsp_prefix .. 'i', vim.lsp.buf.implementation)
vim.keymap.set('n', lsp_prefix .. 'h', vim.lsp.buf.hover)
vim.keymap.set('n', lsp_prefix .. 'n', vim.lsp.buf.rename)
vim.keymap.set('n', lsp_prefix .. 'r', vim.lsp.buf.references)
vim.keymap.set('n', lsp_prefix .. 't', vim.lsp.buf.type_definition)

-- Ref: https://github.com/neovim/nvim-lspconfig/issues/662#issuecomment-983423169
vim.diagnostic.config({ signs = false })

vim.api.nvim_set_hl(0, 'DiagnosticUnderlineError', { underline = true, ctermfg = 'DarkRed' })
vim.api.nvim_set_hl(0, 'DiagnosticUnderlineWarn', { underline = true, ctermfg = 'DarkYellow' })

-- The menu shows a single match too, and no match is picked until picked by hand
vim.opt.completeopt:append({ 'menuone', 'noselect' })
vim.api.nvim_create_autocmd('LspAttach', {
    callback = function(args)
        local client = assert(vim.lsp.get_client_by_id(args.data.client_id))
        if client:supports_method('textDocument/completion') then
            -- Accepting a match also applies its snippet and the server's extra edits
            vim.lsp.completion.enable(true, client.id, args.buf)
            -- The menu opens as you type, with the server's matches only
            vim.bo[args.buf].complete = 'o'
            vim.bo[args.buf].autocomplete = true
        end
    end,
})
-- Enter accepts a match picked by hand, the way <C-y> does, which also expands its snippet, and otherwise starts a new line
vim.keymap.set('i', '<CR>', function()
    if vim.fn.complete_info({ 'selected' }).selected == -1 then return '<CR>' end
    return '<C-y>'
end, { expr = true })

vim.keymap.set('n', lsp_prefix .. 's', require('x.modules.lsp-symbols').open)
