interface SecondModule;
    method Bool isSecondModuleHookedUp;
endinterface

module mkSecondModule(SecondModule);
    method Bool isSecondModuleHookedUp;
        return True;
    endmethod
endmodule
