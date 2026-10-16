vim.api.nvim_create_user_command('GitActiveFiles', function()
    vim.cmd('terminal git active-files')
end, {})

vim.api.nvim_create_user_command('GitRecentFiles', function()
    vim.cmd('terminal git recent-files')
end, {})

vim.api.nvim_create_user_command('GitStatus', function()
    vim.cmd('terminal git status --short')
end, {})
