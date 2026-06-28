-- TODO: This config kind of works for now, but probably can be improved
return function(cmp_nvim_lsp)
    vim.lsp.config('rust_analyzer', {
        cmd = { 'rust-analyzer' },
        filetypes = { 'rust' },
        root_markers = { '.' },
        settings = {
            ['rust-analyzer'] = {
                imports = {
                    granularity = {
                        group = "module",
                    },
                    prefix = "self",
                },
                cargo = {
                    buildScripts = {
                        enable = true,
                    },
                },
                procMacro = {
                    enable = true
                },
            },
        },
    })
    vim.lsp.enable('rust_analyzer')
end
