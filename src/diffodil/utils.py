import logging

from rich.logging import RichHandler


def setup_logger(logger: logging.Logger, verbosity: int):
    logger.addHandler(
        RichHandler(
            markup=True,
            log_time_format="[%X]",
            omit_repeated_times=False,
            show_path=False,
        )
    )

    if verbosity < 1:
        logger.setLevel(logging.WARNING)
    elif verbosity < 2:
        logger.setLevel(logging.INFO)
    else:
        logger.setLevel(logging.DEBUG)
