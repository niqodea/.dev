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
    -- `Explore .` does nothing inside a netrw listing, so the folder is named in full
    local cwd = vim.fn.getcwd()
    explore(vim.fn.fnameescape(cwd))
end

local function explore_git_root()
    local handle = io.popen('git rev-parse --show-toplevel 2> /dev/null')
    local git_root = handle:read('*line')
    handle:close()

    if git_root == nil then error('Not in a git repository') end

    explore(vim.fn.fnameescape(git_root))
end

local function explore_directory()
    local name = vim.fn.expand('%:t')
    explore('')

    for i, line in ipairs(vim.api.nvim_buf_get_lines(0, 0, -1, false)) do
        if
            line == name
            or line == name .. '*' -- executable
            or vim.startswith(line, name .. '@\t') -- symlink, followed by its target
        then
            vim.api.nvim_win_set_cursor(0, { i, 0 })
            return
        end
    end
end

local function explore_path(args) explore(args.args) end

local function browse_up_directory()
    if vim.fn.fnamemodify(vim.b.netrw_curdir, ':p:h:h') == '/' then
        explore(ROOT)
        return
    end

    vim.cmd("normal! m'")
    vim.cmd('execute "normal \\<Plug>NetrwBrowseUpDir"')
end

local function rename_without_overwriting()
    local line = vim.api.nvim_get_current_line()
    -- Strip the marks netrw puts after a name: / for a directory, * for an executable, and @ for a
    -- symlink, followed by its target
    local name = line:gsub('@\t.*$', ''):gsub('[/*]$', '')
    local old_path = vim.fs.joinpath(vim.b.netrw_curdir, name)
    local new_path = vim.fn.input('Moving ' .. old_path .. ' to : ', old_path, 'file')

    if new_path == '' or new_path == old_path then return end
    if vim.uv.fs_stat(new_path) ~= nil then
        vim.notify('Already exists: ' .. new_path, vim.log.levels.ERROR)
        return
    end

    -- rename() copies across file systems, where vim.uv.fs_rename() fails
    if vim.fn.rename(old_path, new_path) ~= 0 then
        vim.notify('Could not move ' .. old_path .. ' to ' .. new_path, vim.log.levels.ERROR)
        return
    end
    vim.cmd('execute "normal \\<Plug>NetrwRefresh"')
end

-- netrw sets its keys on each listing it opens, so ours are set on each listing too
vim.api.nvim_create_autocmd('FileType', {
    pattern = 'netrw',
    callback = function(args)
        -- m' adds the listing you leave to the jumplist, for CTRL-O to come back to
        vim.keymap.set('n', '<CR>', "m'<Plug>NetrwLocalBrowseCheck", { buffer = args.buf, remap = true })
        vim.keymap.set('n', '-', browse_up_directory, { buffer = args.buf })
        -- netrw's R overwrites an existing file without asking
        vim.keymap.set('n', 'R', rename_without_overwriting, { buffer = args.buf })
    end,
})

-- - opens the explorer, the way - goes up a folder inside netrw
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
