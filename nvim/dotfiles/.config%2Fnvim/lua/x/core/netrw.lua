-- Remove instructions banner from netrw
vim.g.netrw_banner = 0
-- Show a directory's past listing instead of rewriting it on every visit. Rewriting it scrambles
-- the lines CTRL-O and CTRL-I remember in it, which gives E19 errors and presses that go nowhere.
-- To rewrite it anyway and get an up-to-date listing, use CTRL-L
vim.g.netrw_fastbrowse = 2
-- / as netrw needs it: netrw drops a trailing slash before looking a directory up, which turns /
-- into an empty path that it reads as the current directory, and // into /
local ROOT = '//'


local function explore(arguments)
    -- m' adds the file you leave to the jumplist, for CTRL-O to come back to
    vim.cmd("normal! m'")
    vim.cmd('Explore ' .. arguments)
end


local function explore_cwd()
    -- `Explore .` won't work when already in netrw for some reason
    local cwd = vim.fn.getcwd()
    explore(vim.fn.fnameescape(cwd))
end


local function explore_git_root()
    local handle = io.popen('git rev-parse --show-toplevel 2> /dev/null')
    local git_root = handle:read('*line')
    handle:close()

    if git_root == nil then
        error('Not in a git repository')
    end

    explore(vim.fn.fnameescape(git_root))
end


local function explore_directory()
    -- Note: This is a hacky, which is inevitable considering we are dealing with netrw
    -- Sometimes, the buffer content stored in lines does not reflect the contents of
    -- netrw, perhaps due to a race condition

    local before_buffer = vim.api.nvim_get_current_buf()
    local before_bufname = vim.fn.expand('%:t')
    explore('')

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


local function explore_path(args)
    explore(args.args)
end


local function browse_up_directory()
    if vim.fn.fnamemodify(vim.b.netrw_curdir, ':p:h:h') == '/' then
        explore(ROOT)
        return
    end

    vim.cmd("normal! m'")
    vim.cmd('execute "normal \\<Plug>NetrwBrowseUpDir"')
end


-- netrw sets its keys on each listing it opens, so ours are set on each listing too
vim.api.nvim_create_autocmd('FileType', {
    pattern = 'netrw',
    callback = function(args)
        -- m' adds the listing you leave to the jumplist, for CTRL-O to come back to
        vim.keymap.set('n', '<CR>', "m'<Plug>NetrwLocalBrowseCheck", { buffer = args.buf, remap = true })
        vim.keymap.set('n', '-', browse_up_directory, { buffer = args.buf })
    end,
})

-- Quick file explorer (we use '-' for consistency with netrw)
vim.api.nvim_create_user_command('ExploreDirectory', explore_directory, {})
vim.api.nvim_create_user_command('ExploreCwd', explore_cwd, {})
vim.api.nvim_create_user_command('ExploreGitRoot', explore_git_root, {})
vim.api.nvim_create_user_command('ExplorePath', explore_path, { nargs = 1 })
vim.keymap.set('n', '<leader>-', ':ExploreDirectory<cr>')
vim.keymap.set('n', '<leader>__', ':ExploreCwd<cr>')
vim.keymap.set('n', '<leader>_g', ':ExploreGitRoot<cr>')
vim.keymap.set('n', '<leader>_v', ':ExplorePath $VIRTUAL_ENV<cr>')
vim.keymap.set('n', '<leader>_~', ':ExplorePath $HOME<cr>')
vim.keymap.set('n', '<leader>_/', ':ExplorePath ' .. ROOT .. '<cr>')
