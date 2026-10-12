return function(set_qflist, set_loclist)

    local mypy_title = 'Mypy'
    local mypy_base_command = 'mypy --show-column-numbers --no-error-summary '
    vim.api.nvim_create_user_command('PythonMypy', function()
        local file_path = vim.fn.expand('%:p')
        local command = mypy_base_command .. vim.fn.shellescape(file_path)
        set_loclist(mypy_title, command)
    end, {})
    vim.api.nvim_create_user_command('PythonMypyProject', function()
        local command = mypy_base_command .. vim.fn.shellescape(vim.fn.getcwd())
        set_qflist(mypy_title, command)
    end, {})

    local ruff_title = 'Ruff'
    local mypy_base_command = 'ruff check --output-format=concise '
    vim.api.nvim_create_user_command('PythonRuff', function()
        local file_path = vim.fn.expand('%:p')
        local command = mypy_base_command .. vim.fn.shellescape(file_path)
        set_loclist(ruff_title, command)
    end, {})
    vim.api.nvim_create_user_command('PythonRuffProject', function()
        local command = mypy_base_command .. vim.fn.shellescape(vim.fn.getcwd())
        set_qflist(ruff_title, command)
    end, {})

end
