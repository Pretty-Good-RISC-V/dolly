interface EmbeddedModule;
    method Bool isEmbeddedModuleHookedUp;
endinterface

module mkEmbeddedModule(EmbeddedModule);
    method Bool isEmbeddedModuleHookedUp;
        return True;
    endmethod
endmodule
