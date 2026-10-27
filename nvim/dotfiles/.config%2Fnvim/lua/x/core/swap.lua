local function recover(base)
    local user = vim.uv.os_get_passwd().username
    local home = vim.uv.os_homedir()
    -- swapfilelist() repeats the directory in every path it returns, so we list it ourselves
    -- Ref: https://github.com/vim/vim/issues/16155
    local swaps = vim.iter(vim.opt.directory:get())
        :map(function(directory) return vim.fn.glob(vim.fs.joinpath(directory, '*'), false, true) end)
        :flatten()
        :totable()
    local qf_list = {}

    for _, swap in ipairs(swaps) do
        local info = vim.fn.swapinfo(swap)
        assert(info.error == nil, 'Cannot read swap file ' .. swap)
        -- Swap files name a file under the home directory as ~user/...
        local path = info.fname:gsub('^~' .. vim.pesc(user) .. '/', home .. '/')

        if
            -- swapinfo() reports a pid only while the nvim that owns the swap file is running
            info.pid > 0
            -- Buffers without a file, like directory listings, leave swap files without a name
            or info.fname == ''
            or vim.fs.relpath(base, path) == nil
        then
            goto continue
        end

        if info.dirty == 0 then
            assert(os.remove(swap))
            goto continue
        end

        local bufnr = vim.fn.bufadd(path)
        if vim.bo[bufnr].modified then
            table.insert(
                qf_list,
                { bufnr = bufnr, lnum = 1, text = 'buffer modified, could not recover from ' .. swap }
            )
            goto continue
        end

        vim.api.nvim_buf_call(bufnr, function()
            local ok, message = pcall(vim.cmd, 'silent recover ' .. vim.fn.fnameescape(swap))
            -- E308 warns that the file changed on disk after the crash, once the recovery is done
            if not ok and not message:find('E308:') then error(message) end
        end)
        vim.bo[bufnr].buflisted = true
        assert(os.remove(swap))
        table.insert(qf_list, { bufnr = bufnr, lnum = 1, text = 'recovered, :write to keep or :edit! to drop' })

        ::continue::
    end

    if #qf_list == 0 then
        vim.notify('Nothing to recover')
        return
    end
    vim.fn.setqflist(qf_list)
    vim.cmd('copen')
end

local function recover_cwd() recover(vim.fn.getcwd()) end

local function recover_path(args) recover(vim.fs.abspath(vim.fn.expand(args.args))) end

-- Recover what crashed nvims left unsaved in their swap files, and remove the swap files
vim.api.nvim_create_user_command('RecoverCwd', recover_cwd, {})
vim.api.nvim_create_user_command('RecoverPath', recover_path, { nargs = 1, complete = 'dir' })
