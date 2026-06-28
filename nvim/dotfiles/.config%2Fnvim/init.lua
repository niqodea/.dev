vim.api.nvim_create_user_command('OwnStart', function()
    require('own')
end, {})
