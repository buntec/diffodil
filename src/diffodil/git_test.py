import asyncio

from diffodil.git import GitFlags, get_git_diff, get_git_log, git_diff_compact_summary


async def main():
    flags = GitFlags()
    # logs = await get_git_log('/home/buntec/repos/github/llama.cpp')
    diff = await get_git_diff(
        "/home/buntec/repos/watch-my-cpp/", "0eabd28", "81c8b1c", flags
    )

    print(diff)

    stats = await git_diff_compact_summary(
        "/home/buntec/repos/watch-my-cpp/", "HEAD~5", "HEAD"
    )

    print(stats)


asyncio.run(main())
