# Sysinspect Objects
# Author: Bo Maryniuk <bo@maryniuk.net>

from __future__ import annotations
from typing import Any
import json


class SysinspectReturn:
    """
    Return structure for the Sysinspect module.
    """

    def __init__(self, retcode: int = 0, message: str = ""):
        """
        Constructor.
        """
        self.retcode = retcode
        self.warnings = []
        self.message = message
        self.data = {}

    def set_retcode(self, retcode: int = 0) -> SysinspectReturn:
        """
        Set return code
        """
        self.retcode = retcode
        return self

    def set_message(self, message: str) -> SysinspectReturn:
        """
        Set message
        """
        self.message = message
        return self

    def add_warning(self, warning: str) -> SysinspectReturn:
        """
        Add a warning message
        """
        self.warnings.append(warning)
        return self

    def add_data(self, data: dict[str, Any]) -> SysinspectReturn:
        """
        Add data structure
        """
        if data:
            self.data.update(data)
        return self


    def __str__(self):
        ret:dict = {
            "retcode": self.retcode,
            "message": self.message,
        }

        if self.warnings:
            ret["warning"] = self.warnings

        if self.data:
            ret["data"] = self.data

        return json.dumps(ret)
    
