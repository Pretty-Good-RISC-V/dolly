//!submodule another_module
//!submodule second_module
import AnotherModule::*;
import SecondModule::*;

interface Simple;
    method Bool isHookedUp;
endinterface

//!topmodule mkSimple
module mkSimple(Simple);
    AnotherModule anotherModule <- mkAnotherModule;
    SecondModule secondModule <- mkSecondModule;

    method Bool isHookedUp;
        return anotherModule.isAnotherModuleHookedUp && secondModule.isSecondModuleHookedUp;
    endmethod
endmodule
