local M = {}

vim.cmd('packadd nvim-snippy')
M.snippy = require('snippy')

M.snippy.setup{
    mappings = {
        is = {
            ['<C-s>'] = M.snippy.mapping.Expand,
            ['<Tab>'] = M.snippy.mapping.Next,
            ['<S-Tab>'] = M.snippy.mapping.Previous,
        },
    },
}

return M
