local M = {}


function M.yank_path()
    local path = vim.fn.expand('%:.')
    vim.fn.setreg('+', path)
end


function M.explore_cwd()
    -- `Explore .` won't work when already in netrw for some reason
    local cwd = vim.fn.getcwd()
    vim.cmd('Explore ' .. cwd)
end


function M.explore_git_root()
    local handle = io.popen('git rev-parse --show-toplevel 2> /dev/null')
    local git_root = handle:read('*line')
    handle:close()

    if git_root == nil then
        error('Not in a git repository')
    end

    vim.cmd('Explore ' .. git_root)
end


function M.explore_directory()
    -- Note: This is a hacky, which is inevitable considering we are dealing with netrw
    -- Sometimes, the buffer content stored in lines does not reflect the contents of
    -- netrw, perhaps due to a race condition

    local before_buffer = vim.api.nvim_get_current_buf()
    local before_bufname = vim.fn.expand('%:t')
    vim.cmd('Explore')

    if vim.api.nvim_get_current_buf() == before_buffer then
        return  -- netrw already open in a directory
    end

    local lines = vim.fn.getbufline(0, 1, '$')
    if #lines < 2 or lines[1] ~= '../' or lines[2] ~= './' then
        -- Two main reasons why this could happen:
        -- 1) netrw is still loading
        -- 2) netrw liststyle is not set to 'thin'
        return
    end

    -- We are now reasonably sure `lines` contains netrw contents with liststyle 'thin'

    -- Position cursor on the previous buffer's file
    for i, line in ipairs(lines) do

        if (
            line == before_bufname or  -- normal file
            line == before_bufname .. '*' or  -- executable
            line:match('^.*@\t') == before_bufname .. '@\t'  -- symlink
        ) then
            vim.api.nvim_win_set_cursor(0, {i, 0})
            return
        end

        ::continue::
    end

    -- If we reach here, the file associated to the buffer does not exist
end


function M.reload_buffers()
    local modified_bufnrs = {}
    for _, bufnr in ipairs(vim.api.nvim_list_bufs()) do
        if vim.api.nvim_buf_is_loaded(bufnr) == false then
            goto continue
        end
        if vim.api.nvim_buf_get_option(bufnr, 'buftype') ~= '' then
            goto continue
        end
        if vim.api.nvim_buf_get_name(bufnr) == '' then
            goto continue
        end
        if vim.api.nvim_buf_get_option(bufnr, 'modified') then
            table.insert(modified_bufnrs, bufnr)
            goto continue
        end
        vim.api.nvim_buf_call(bufnr, function()
            -- NOTE: editing a buffer with nvim_buf_call has the following issue:
            -- https://github.com/neovim/neovim/issues/25877
            -- For now, as a hack, we temporarily disable matchparen in the following way
            local matchpairs = vim.o.matchpairs
            vim.o.matchpairs = ""

            vim.cmd('edit')

            vim.o.matchpairs = matchpairs
        end)

        ::continue::
    end

    if #modified_bufnrs > 0 then
        local qf_list = {}
        for _, bufnr in ipairs(modified_bufnrs) do
            local filename = vim.api.nvim_buf_get_name(bufnr)
            table.insert(
                qf_list,
                {
                    bufnr = bufnr,
                    filename = filename,
                    lnum = 1,
                    col = 1,
                    text = "buffer modified, could not read from file",
                }
            )
        end
        vim.fn.setqflist(qf_list)
        vim.cmd('copen')
    end
end


return M
