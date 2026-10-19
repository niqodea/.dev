vim.lsp.config('ty', {
    cmd = { 'ty', 'server' },
    filetypes = { 'python' },
    root_markers = { '.' },
})
vim.lsp.enable('ty')

local ropefolder = require('x.utils').get_atdir() .. '/ropeproject'

vim.api.nvim_create_user_command('PythonRopifyMoveModule', function()
    local resource = vim.fn.expand('%')
    local command = 'ropify move-module ' .. vim.fn.shellescape(resource) .. ' --ropefolder ' .. vim.fn.shellescape(ropefolder)
    vim.cmd('split | terminal ' .. command)
    vim.bo.buflisted = false
end, {})

vim.api.nvim_create_user_command('PythonRopifyMoveSymbol', function()
    local resource = vim.fn.expand('%')
    local offset = vim.fn.line2byte(vim.fn.line('.')) + vim.fn.col('.') - 1
    local command = 'ropify move-symbol ' .. vim.fn.shellescape(resource) .. ' ' .. offset .. ' --ropefolder ' .. vim.fn.shellescape(ropefolder)
    vim.cmd('split | terminal ' .. command)
    vim.bo.buflisted = false
end, {})

vim.api.nvim_create_user_command('PythonRopifyShowImports', function()
    local name = vim.fn.expand('<cword>')
    local command = 'ropify show-imports ' .. vim.fn.shellescape(name) .. ' --ropefolder ' .. vim.fn.shellescape(ropefolder)
    vim.cmd('split | terminal ' .. command)
    vim.bo.buflisted = false
end, {})
