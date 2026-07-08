"""Benchmark stress fixture for `narrowing_typeis_2`.

Repeats numbered variants of `TypeIs[X]` return annotations where `X` is
not consistent with the input parameter type (PEP 742: the narrowed type
must be a subtype of the input type). Every variant is an error site.
"""

from typing_extensions import TypeIs

def check_str0(value0: int) -> TypeIs[str]:
    return False

def check_items0(items0: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha0: ...
class Beta0: ...

def check_alpha0(obj0: Alpha0) -> TypeIs[Beta0]:
    return False

class Validator0:
    def is_str_map(self, data0: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str1(value1: int) -> TypeIs[str]:
    return False

def check_items1(items1: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha1: ...
class Beta1: ...

def check_alpha1(obj1: Alpha1) -> TypeIs[Beta1]:
    return False

class Validator1:
    def is_str_map(self, data1: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str2(value2: int) -> TypeIs[str]:
    return False

def check_items2(items2: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha2: ...
class Beta2: ...

def check_alpha2(obj2: Alpha2) -> TypeIs[Beta2]:
    return False

class Validator2:
    def is_str_map(self, data2: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str3(value3: int) -> TypeIs[str]:
    return False

def check_items3(items3: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha3: ...
class Beta3: ...

def check_alpha3(obj3: Alpha3) -> TypeIs[Beta3]:
    return False

class Validator3:
    def is_str_map(self, data3: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str4(value4: int) -> TypeIs[str]:
    return False

def check_items4(items4: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha4: ...
class Beta4: ...

def check_alpha4(obj4: Alpha4) -> TypeIs[Beta4]:
    return False

class Validator4:
    def is_str_map(self, data4: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str5(value5: int) -> TypeIs[str]:
    return False

def check_items5(items5: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha5: ...
class Beta5: ...

def check_alpha5(obj5: Alpha5) -> TypeIs[Beta5]:
    return False

class Validator5:
    def is_str_map(self, data5: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str6(value6: int) -> TypeIs[str]:
    return False

def check_items6(items6: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha6: ...
class Beta6: ...

def check_alpha6(obj6: Alpha6) -> TypeIs[Beta6]:
    return False

class Validator6:
    def is_str_map(self, data6: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str7(value7: int) -> TypeIs[str]:
    return False

def check_items7(items7: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha7: ...
class Beta7: ...

def check_alpha7(obj7: Alpha7) -> TypeIs[Beta7]:
    return False

class Validator7:
    def is_str_map(self, data7: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str8(value8: int) -> TypeIs[str]:
    return False

def check_items8(items8: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha8: ...
class Beta8: ...

def check_alpha8(obj8: Alpha8) -> TypeIs[Beta8]:
    return False

class Validator8:
    def is_str_map(self, data8: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str9(value9: int) -> TypeIs[str]:
    return False

def check_items9(items9: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha9: ...
class Beta9: ...

def check_alpha9(obj9: Alpha9) -> TypeIs[Beta9]:
    return False

class Validator9:
    def is_str_map(self, data9: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str10(value10: int) -> TypeIs[str]:
    return False

def check_items10(items10: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha10: ...
class Beta10: ...

def check_alpha10(obj10: Alpha10) -> TypeIs[Beta10]:
    return False

class Validator10:
    def is_str_map(self, data10: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str11(value11: int) -> TypeIs[str]:
    return False

def check_items11(items11: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha11: ...
class Beta11: ...

def check_alpha11(obj11: Alpha11) -> TypeIs[Beta11]:
    return False

class Validator11:
    def is_str_map(self, data11: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str12(value12: int) -> TypeIs[str]:
    return False

def check_items12(items12: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha12: ...
class Beta12: ...

def check_alpha12(obj12: Alpha12) -> TypeIs[Beta12]:
    return False

class Validator12:
    def is_str_map(self, data12: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str13(value13: int) -> TypeIs[str]:
    return False

def check_items13(items13: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha13: ...
class Beta13: ...

def check_alpha13(obj13: Alpha13) -> TypeIs[Beta13]:
    return False

class Validator13:
    def is_str_map(self, data13: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str14(value14: int) -> TypeIs[str]:
    return False

def check_items14(items14: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha14: ...
class Beta14: ...

def check_alpha14(obj14: Alpha14) -> TypeIs[Beta14]:
    return False

class Validator14:
    def is_str_map(self, data14: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str15(value15: int) -> TypeIs[str]:
    return False

def check_items15(items15: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha15: ...
class Beta15: ...

def check_alpha15(obj15: Alpha15) -> TypeIs[Beta15]:
    return False

class Validator15:
    def is_str_map(self, data15: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str16(value16: int) -> TypeIs[str]:
    return False

def check_items16(items16: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha16: ...
class Beta16: ...

def check_alpha16(obj16: Alpha16) -> TypeIs[Beta16]:
    return False

class Validator16:
    def is_str_map(self, data16: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str17(value17: int) -> TypeIs[str]:
    return False

def check_items17(items17: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha17: ...
class Beta17: ...

def check_alpha17(obj17: Alpha17) -> TypeIs[Beta17]:
    return False

class Validator17:
    def is_str_map(self, data17: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str18(value18: int) -> TypeIs[str]:
    return False

def check_items18(items18: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha18: ...
class Beta18: ...

def check_alpha18(obj18: Alpha18) -> TypeIs[Beta18]:
    return False

class Validator18:
    def is_str_map(self, data18: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str19(value19: int) -> TypeIs[str]:
    return False

def check_items19(items19: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha19: ...
class Beta19: ...

def check_alpha19(obj19: Alpha19) -> TypeIs[Beta19]:
    return False

class Validator19:
    def is_str_map(self, data19: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str20(value20: int) -> TypeIs[str]:
    return False

def check_items20(items20: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha20: ...
class Beta20: ...

def check_alpha20(obj20: Alpha20) -> TypeIs[Beta20]:
    return False

class Validator20:
    def is_str_map(self, data20: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str21(value21: int) -> TypeIs[str]:
    return False

def check_items21(items21: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha21: ...
class Beta21: ...

def check_alpha21(obj21: Alpha21) -> TypeIs[Beta21]:
    return False

class Validator21:
    def is_str_map(self, data21: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str22(value22: int) -> TypeIs[str]:
    return False

def check_items22(items22: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha22: ...
class Beta22: ...

def check_alpha22(obj22: Alpha22) -> TypeIs[Beta22]:
    return False

class Validator22:
    def is_str_map(self, data22: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str23(value23: int) -> TypeIs[str]:
    return False

def check_items23(items23: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha23: ...
class Beta23: ...

def check_alpha23(obj23: Alpha23) -> TypeIs[Beta23]:
    return False

class Validator23:
    def is_str_map(self, data23: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str24(value24: int) -> TypeIs[str]:
    return False

def check_items24(items24: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha24: ...
class Beta24: ...

def check_alpha24(obj24: Alpha24) -> TypeIs[Beta24]:
    return False

class Validator24:
    def is_str_map(self, data24: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str25(value25: int) -> TypeIs[str]:
    return False

def check_items25(items25: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha25: ...
class Beta25: ...

def check_alpha25(obj25: Alpha25) -> TypeIs[Beta25]:
    return False

class Validator25:
    def is_str_map(self, data25: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str26(value26: int) -> TypeIs[str]:
    return False

def check_items26(items26: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha26: ...
class Beta26: ...

def check_alpha26(obj26: Alpha26) -> TypeIs[Beta26]:
    return False

class Validator26:
    def is_str_map(self, data26: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str27(value27: int) -> TypeIs[str]:
    return False

def check_items27(items27: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha27: ...
class Beta27: ...

def check_alpha27(obj27: Alpha27) -> TypeIs[Beta27]:
    return False

class Validator27:
    def is_str_map(self, data27: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str28(value28: int) -> TypeIs[str]:
    return False

def check_items28(items28: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha28: ...
class Beta28: ...

def check_alpha28(obj28: Alpha28) -> TypeIs[Beta28]:
    return False

class Validator28:
    def is_str_map(self, data28: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str29(value29: int) -> TypeIs[str]:
    return False

def check_items29(items29: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha29: ...
class Beta29: ...

def check_alpha29(obj29: Alpha29) -> TypeIs[Beta29]:
    return False

class Validator29:
    def is_str_map(self, data29: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str30(value30: int) -> TypeIs[str]:
    return False

def check_items30(items30: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha30: ...
class Beta30: ...

def check_alpha30(obj30: Alpha30) -> TypeIs[Beta30]:
    return False

class Validator30:
    def is_str_map(self, data30: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str31(value31: int) -> TypeIs[str]:
    return False

def check_items31(items31: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha31: ...
class Beta31: ...

def check_alpha31(obj31: Alpha31) -> TypeIs[Beta31]:
    return False

class Validator31:
    def is_str_map(self, data31: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str32(value32: int) -> TypeIs[str]:
    return False

def check_items32(items32: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha32: ...
class Beta32: ...

def check_alpha32(obj32: Alpha32) -> TypeIs[Beta32]:
    return False

class Validator32:
    def is_str_map(self, data32: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str33(value33: int) -> TypeIs[str]:
    return False

def check_items33(items33: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha33: ...
class Beta33: ...

def check_alpha33(obj33: Alpha33) -> TypeIs[Beta33]:
    return False

class Validator33:
    def is_str_map(self, data33: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str34(value34: int) -> TypeIs[str]:
    return False

def check_items34(items34: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha34: ...
class Beta34: ...

def check_alpha34(obj34: Alpha34) -> TypeIs[Beta34]:
    return False

class Validator34:
    def is_str_map(self, data34: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str35(value35: int) -> TypeIs[str]:
    return False

def check_items35(items35: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha35: ...
class Beta35: ...

def check_alpha35(obj35: Alpha35) -> TypeIs[Beta35]:
    return False

class Validator35:
    def is_str_map(self, data35: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str36(value36: int) -> TypeIs[str]:
    return False

def check_items36(items36: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha36: ...
class Beta36: ...

def check_alpha36(obj36: Alpha36) -> TypeIs[Beta36]:
    return False

class Validator36:
    def is_str_map(self, data36: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str37(value37: int) -> TypeIs[str]:
    return False

def check_items37(items37: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha37: ...
class Beta37: ...

def check_alpha37(obj37: Alpha37) -> TypeIs[Beta37]:
    return False

class Validator37:
    def is_str_map(self, data37: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str38(value38: int) -> TypeIs[str]:
    return False

def check_items38(items38: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha38: ...
class Beta38: ...

def check_alpha38(obj38: Alpha38) -> TypeIs[Beta38]:
    return False

class Validator38:
    def is_str_map(self, data38: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str39(value39: int) -> TypeIs[str]:
    return False

def check_items39(items39: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha39: ...
class Beta39: ...

def check_alpha39(obj39: Alpha39) -> TypeIs[Beta39]:
    return False

class Validator39:
    def is_str_map(self, data39: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str40(value40: int) -> TypeIs[str]:
    return False

def check_items40(items40: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha40: ...
class Beta40: ...

def check_alpha40(obj40: Alpha40) -> TypeIs[Beta40]:
    return False

class Validator40:
    def is_str_map(self, data40: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str41(value41: int) -> TypeIs[str]:
    return False

def check_items41(items41: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha41: ...
class Beta41: ...

def check_alpha41(obj41: Alpha41) -> TypeIs[Beta41]:
    return False

class Validator41:
    def is_str_map(self, data41: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str42(value42: int) -> TypeIs[str]:
    return False

def check_items42(items42: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha42: ...
class Beta42: ...

def check_alpha42(obj42: Alpha42) -> TypeIs[Beta42]:
    return False

class Validator42:
    def is_str_map(self, data42: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str43(value43: int) -> TypeIs[str]:
    return False

def check_items43(items43: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha43: ...
class Beta43: ...

def check_alpha43(obj43: Alpha43) -> TypeIs[Beta43]:
    return False

class Validator43:
    def is_str_map(self, data43: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str44(value44: int) -> TypeIs[str]:
    return False

def check_items44(items44: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha44: ...
class Beta44: ...

def check_alpha44(obj44: Alpha44) -> TypeIs[Beta44]:
    return False

class Validator44:
    def is_str_map(self, data44: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str45(value45: int) -> TypeIs[str]:
    return False

def check_items45(items45: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha45: ...
class Beta45: ...

def check_alpha45(obj45: Alpha45) -> TypeIs[Beta45]:
    return False

class Validator45:
    def is_str_map(self, data45: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str46(value46: int) -> TypeIs[str]:
    return False

def check_items46(items46: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha46: ...
class Beta46: ...

def check_alpha46(obj46: Alpha46) -> TypeIs[Beta46]:
    return False

class Validator46:
    def is_str_map(self, data46: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str47(value47: int) -> TypeIs[str]:
    return False

def check_items47(items47: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha47: ...
class Beta47: ...

def check_alpha47(obj47: Alpha47) -> TypeIs[Beta47]:
    return False

class Validator47:
    def is_str_map(self, data47: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str48(value48: int) -> TypeIs[str]:
    return False

def check_items48(items48: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha48: ...
class Beta48: ...

def check_alpha48(obj48: Alpha48) -> TypeIs[Beta48]:
    return False

class Validator48:
    def is_str_map(self, data48: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str49(value49: int) -> TypeIs[str]:
    return False

def check_items49(items49: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha49: ...
class Beta49: ...

def check_alpha49(obj49: Alpha49) -> TypeIs[Beta49]:
    return False

class Validator49:
    def is_str_map(self, data49: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str50(value50: int) -> TypeIs[str]:
    return False

def check_items50(items50: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha50: ...
class Beta50: ...

def check_alpha50(obj50: Alpha50) -> TypeIs[Beta50]:
    return False

class Validator50:
    def is_str_map(self, data50: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str51(value51: int) -> TypeIs[str]:
    return False

def check_items51(items51: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha51: ...
class Beta51: ...

def check_alpha51(obj51: Alpha51) -> TypeIs[Beta51]:
    return False

class Validator51:
    def is_str_map(self, data51: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str52(value52: int) -> TypeIs[str]:
    return False

def check_items52(items52: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha52: ...
class Beta52: ...

def check_alpha52(obj52: Alpha52) -> TypeIs[Beta52]:
    return False

class Validator52:
    def is_str_map(self, data52: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str53(value53: int) -> TypeIs[str]:
    return False

def check_items53(items53: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha53: ...
class Beta53: ...

def check_alpha53(obj53: Alpha53) -> TypeIs[Beta53]:
    return False

class Validator53:
    def is_str_map(self, data53: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str54(value54: int) -> TypeIs[str]:
    return False

def check_items54(items54: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha54: ...
class Beta54: ...

def check_alpha54(obj54: Alpha54) -> TypeIs[Beta54]:
    return False

class Validator54:
    def is_str_map(self, data54: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str55(value55: int) -> TypeIs[str]:
    return False

def check_items55(items55: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha55: ...
class Beta55: ...

def check_alpha55(obj55: Alpha55) -> TypeIs[Beta55]:
    return False

class Validator55:
    def is_str_map(self, data55: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str56(value56: int) -> TypeIs[str]:
    return False

def check_items56(items56: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha56: ...
class Beta56: ...

def check_alpha56(obj56: Alpha56) -> TypeIs[Beta56]:
    return False

class Validator56:
    def is_str_map(self, data56: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str57(value57: int) -> TypeIs[str]:
    return False

def check_items57(items57: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha57: ...
class Beta57: ...

def check_alpha57(obj57: Alpha57) -> TypeIs[Beta57]:
    return False

class Validator57:
    def is_str_map(self, data57: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str58(value58: int) -> TypeIs[str]:
    return False

def check_items58(items58: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha58: ...
class Beta58: ...

def check_alpha58(obj58: Alpha58) -> TypeIs[Beta58]:
    return False

class Validator58:
    def is_str_map(self, data58: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str59(value59: int) -> TypeIs[str]:
    return False

def check_items59(items59: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha59: ...
class Beta59: ...

def check_alpha59(obj59: Alpha59) -> TypeIs[Beta59]:
    return False

class Validator59:
    def is_str_map(self, data59: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str60(value60: int) -> TypeIs[str]:
    return False

def check_items60(items60: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha60: ...
class Beta60: ...

def check_alpha60(obj60: Alpha60) -> TypeIs[Beta60]:
    return False

class Validator60:
    def is_str_map(self, data60: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str61(value61: int) -> TypeIs[str]:
    return False

def check_items61(items61: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha61: ...
class Beta61: ...

def check_alpha61(obj61: Alpha61) -> TypeIs[Beta61]:
    return False

class Validator61:
    def is_str_map(self, data61: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str62(value62: int) -> TypeIs[str]:
    return False

def check_items62(items62: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha62: ...
class Beta62: ...

def check_alpha62(obj62: Alpha62) -> TypeIs[Beta62]:
    return False

class Validator62:
    def is_str_map(self, data62: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str63(value63: int) -> TypeIs[str]:
    return False

def check_items63(items63: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha63: ...
class Beta63: ...

def check_alpha63(obj63: Alpha63) -> TypeIs[Beta63]:
    return False

class Validator63:
    def is_str_map(self, data63: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str64(value64: int) -> TypeIs[str]:
    return False

def check_items64(items64: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha64: ...
class Beta64: ...

def check_alpha64(obj64: Alpha64) -> TypeIs[Beta64]:
    return False

class Validator64:
    def is_str_map(self, data64: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str65(value65: int) -> TypeIs[str]:
    return False

def check_items65(items65: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha65: ...
class Beta65: ...

def check_alpha65(obj65: Alpha65) -> TypeIs[Beta65]:
    return False

class Validator65:
    def is_str_map(self, data65: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str66(value66: int) -> TypeIs[str]:
    return False

def check_items66(items66: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha66: ...
class Beta66: ...

def check_alpha66(obj66: Alpha66) -> TypeIs[Beta66]:
    return False

class Validator66:
    def is_str_map(self, data66: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str67(value67: int) -> TypeIs[str]:
    return False

def check_items67(items67: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha67: ...
class Beta67: ...

def check_alpha67(obj67: Alpha67) -> TypeIs[Beta67]:
    return False

class Validator67:
    def is_str_map(self, data67: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str68(value68: int) -> TypeIs[str]:
    return False

def check_items68(items68: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha68: ...
class Beta68: ...

def check_alpha68(obj68: Alpha68) -> TypeIs[Beta68]:
    return False

class Validator68:
    def is_str_map(self, data68: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str69(value69: int) -> TypeIs[str]:
    return False

def check_items69(items69: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha69: ...
class Beta69: ...

def check_alpha69(obj69: Alpha69) -> TypeIs[Beta69]:
    return False

class Validator69:
    def is_str_map(self, data69: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str70(value70: int) -> TypeIs[str]:
    return False

def check_items70(items70: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha70: ...
class Beta70: ...

def check_alpha70(obj70: Alpha70) -> TypeIs[Beta70]:
    return False

class Validator70:
    def is_str_map(self, data70: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str71(value71: int) -> TypeIs[str]:
    return False

def check_items71(items71: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha71: ...
class Beta71: ...

def check_alpha71(obj71: Alpha71) -> TypeIs[Beta71]:
    return False

class Validator71:
    def is_str_map(self, data71: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str72(value72: int) -> TypeIs[str]:
    return False

def check_items72(items72: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha72: ...
class Beta72: ...

def check_alpha72(obj72: Alpha72) -> TypeIs[Beta72]:
    return False

class Validator72:
    def is_str_map(self, data72: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str73(value73: int) -> TypeIs[str]:
    return False

def check_items73(items73: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha73: ...
class Beta73: ...

def check_alpha73(obj73: Alpha73) -> TypeIs[Beta73]:
    return False

class Validator73:
    def is_str_map(self, data73: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str74(value74: int) -> TypeIs[str]:
    return False

def check_items74(items74: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha74: ...
class Beta74: ...

def check_alpha74(obj74: Alpha74) -> TypeIs[Beta74]:
    return False

class Validator74:
    def is_str_map(self, data74: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str75(value75: int) -> TypeIs[str]:
    return False

def check_items75(items75: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha75: ...
class Beta75: ...

def check_alpha75(obj75: Alpha75) -> TypeIs[Beta75]:
    return False

class Validator75:
    def is_str_map(self, data75: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str76(value76: int) -> TypeIs[str]:
    return False

def check_items76(items76: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha76: ...
class Beta76: ...

def check_alpha76(obj76: Alpha76) -> TypeIs[Beta76]:
    return False

class Validator76:
    def is_str_map(self, data76: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str77(value77: int) -> TypeIs[str]:
    return False

def check_items77(items77: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha77: ...
class Beta77: ...

def check_alpha77(obj77: Alpha77) -> TypeIs[Beta77]:
    return False

class Validator77:
    def is_str_map(self, data77: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str78(value78: int) -> TypeIs[str]:
    return False

def check_items78(items78: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha78: ...
class Beta78: ...

def check_alpha78(obj78: Alpha78) -> TypeIs[Beta78]:
    return False

class Validator78:
    def is_str_map(self, data78: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str79(value79: int) -> TypeIs[str]:
    return False

def check_items79(items79: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha79: ...
class Beta79: ...

def check_alpha79(obj79: Alpha79) -> TypeIs[Beta79]:
    return False

class Validator79:
    def is_str_map(self, data79: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str80(value80: int) -> TypeIs[str]:
    return False

def check_items80(items80: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha80: ...
class Beta80: ...

def check_alpha80(obj80: Alpha80) -> TypeIs[Beta80]:
    return False

class Validator80:
    def is_str_map(self, data80: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str81(value81: int) -> TypeIs[str]:
    return False

def check_items81(items81: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha81: ...
class Beta81: ...

def check_alpha81(obj81: Alpha81) -> TypeIs[Beta81]:
    return False

class Validator81:
    def is_str_map(self, data81: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str82(value82: int) -> TypeIs[str]:
    return False

def check_items82(items82: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha82: ...
class Beta82: ...

def check_alpha82(obj82: Alpha82) -> TypeIs[Beta82]:
    return False

class Validator82:
    def is_str_map(self, data82: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str83(value83: int) -> TypeIs[str]:
    return False

def check_items83(items83: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha83: ...
class Beta83: ...

def check_alpha83(obj83: Alpha83) -> TypeIs[Beta83]:
    return False

class Validator83:
    def is_str_map(self, data83: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str84(value84: int) -> TypeIs[str]:
    return False

def check_items84(items84: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha84: ...
class Beta84: ...

def check_alpha84(obj84: Alpha84) -> TypeIs[Beta84]:
    return False

class Validator84:
    def is_str_map(self, data84: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str85(value85: int) -> TypeIs[str]:
    return False

def check_items85(items85: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha85: ...
class Beta85: ...

def check_alpha85(obj85: Alpha85) -> TypeIs[Beta85]:
    return False

class Validator85:
    def is_str_map(self, data85: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str86(value86: int) -> TypeIs[str]:
    return False

def check_items86(items86: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha86: ...
class Beta86: ...

def check_alpha86(obj86: Alpha86) -> TypeIs[Beta86]:
    return False

class Validator86:
    def is_str_map(self, data86: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str87(value87: int) -> TypeIs[str]:
    return False

def check_items87(items87: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha87: ...
class Beta87: ...

def check_alpha87(obj87: Alpha87) -> TypeIs[Beta87]:
    return False

class Validator87:
    def is_str_map(self, data87: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str88(value88: int) -> TypeIs[str]:
    return False

def check_items88(items88: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha88: ...
class Beta88: ...

def check_alpha88(obj88: Alpha88) -> TypeIs[Beta88]:
    return False

class Validator88:
    def is_str_map(self, data88: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str89(value89: int) -> TypeIs[str]:
    return False

def check_items89(items89: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha89: ...
class Beta89: ...

def check_alpha89(obj89: Alpha89) -> TypeIs[Beta89]:
    return False

class Validator89:
    def is_str_map(self, data89: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str90(value90: int) -> TypeIs[str]:
    return False

def check_items90(items90: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha90: ...
class Beta90: ...

def check_alpha90(obj90: Alpha90) -> TypeIs[Beta90]:
    return False

class Validator90:
    def is_str_map(self, data90: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str91(value91: int) -> TypeIs[str]:
    return False

def check_items91(items91: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha91: ...
class Beta91: ...

def check_alpha91(obj91: Alpha91) -> TypeIs[Beta91]:
    return False

class Validator91:
    def is_str_map(self, data91: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str92(value92: int) -> TypeIs[str]:
    return False

def check_items92(items92: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha92: ...
class Beta92: ...

def check_alpha92(obj92: Alpha92) -> TypeIs[Beta92]:
    return False

class Validator92:
    def is_str_map(self, data92: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str93(value93: int) -> TypeIs[str]:
    return False

def check_items93(items93: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha93: ...
class Beta93: ...

def check_alpha93(obj93: Alpha93) -> TypeIs[Beta93]:
    return False

class Validator93:
    def is_str_map(self, data93: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str94(value94: int) -> TypeIs[str]:
    return False

def check_items94(items94: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha94: ...
class Beta94: ...

def check_alpha94(obj94: Alpha94) -> TypeIs[Beta94]:
    return False

class Validator94:
    def is_str_map(self, data94: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str95(value95: int) -> TypeIs[str]:
    return False

def check_items95(items95: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha95: ...
class Beta95: ...

def check_alpha95(obj95: Alpha95) -> TypeIs[Beta95]:
    return False

class Validator95:
    def is_str_map(self, data95: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str96(value96: int) -> TypeIs[str]:
    return False

def check_items96(items96: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha96: ...
class Beta96: ...

def check_alpha96(obj96: Alpha96) -> TypeIs[Beta96]:
    return False

class Validator96:
    def is_str_map(self, data96: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str97(value97: int) -> TypeIs[str]:
    return False

def check_items97(items97: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha97: ...
class Beta97: ...

def check_alpha97(obj97: Alpha97) -> TypeIs[Beta97]:
    return False

class Validator97:
    def is_str_map(self, data97: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str98(value98: int) -> TypeIs[str]:
    return False

def check_items98(items98: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha98: ...
class Beta98: ...

def check_alpha98(obj98: Alpha98) -> TypeIs[Beta98]:
    return False

class Validator98:
    def is_str_map(self, data98: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str99(value99: int) -> TypeIs[str]:
    return False

def check_items99(items99: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha99: ...
class Beta99: ...

def check_alpha99(obj99: Alpha99) -> TypeIs[Beta99]:
    return False

class Validator99:
    def is_str_map(self, data99: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str100(value100: int) -> TypeIs[str]:
    return False

def check_items100(items100: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha100: ...
class Beta100: ...

def check_alpha100(obj100: Alpha100) -> TypeIs[Beta100]:
    return False

class Validator100:
    def is_str_map(self, data100: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str101(value101: int) -> TypeIs[str]:
    return False

def check_items101(items101: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha101: ...
class Beta101: ...

def check_alpha101(obj101: Alpha101) -> TypeIs[Beta101]:
    return False

class Validator101:
    def is_str_map(self, data101: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str102(value102: int) -> TypeIs[str]:
    return False

def check_items102(items102: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha102: ...
class Beta102: ...

def check_alpha102(obj102: Alpha102) -> TypeIs[Beta102]:
    return False

class Validator102:
    def is_str_map(self, data102: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str103(value103: int) -> TypeIs[str]:
    return False

def check_items103(items103: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha103: ...
class Beta103: ...

def check_alpha103(obj103: Alpha103) -> TypeIs[Beta103]:
    return False

class Validator103:
    def is_str_map(self, data103: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str104(value104: int) -> TypeIs[str]:
    return False

def check_items104(items104: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha104: ...
class Beta104: ...

def check_alpha104(obj104: Alpha104) -> TypeIs[Beta104]:
    return False

class Validator104:
    def is_str_map(self, data104: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str105(value105: int) -> TypeIs[str]:
    return False

def check_items105(items105: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha105: ...
class Beta105: ...

def check_alpha105(obj105: Alpha105) -> TypeIs[Beta105]:
    return False

class Validator105:
    def is_str_map(self, data105: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str106(value106: int) -> TypeIs[str]:
    return False

def check_items106(items106: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha106: ...
class Beta106: ...

def check_alpha106(obj106: Alpha106) -> TypeIs[Beta106]:
    return False

class Validator106:
    def is_str_map(self, data106: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str107(value107: int) -> TypeIs[str]:
    return False

def check_items107(items107: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha107: ...
class Beta107: ...

def check_alpha107(obj107: Alpha107) -> TypeIs[Beta107]:
    return False

class Validator107:
    def is_str_map(self, data107: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str108(value108: int) -> TypeIs[str]:
    return False

def check_items108(items108: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha108: ...
class Beta108: ...

def check_alpha108(obj108: Alpha108) -> TypeIs[Beta108]:
    return False

class Validator108:
    def is_str_map(self, data108: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str109(value109: int) -> TypeIs[str]:
    return False

def check_items109(items109: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha109: ...
class Beta109: ...

def check_alpha109(obj109: Alpha109) -> TypeIs[Beta109]:
    return False

class Validator109:
    def is_str_map(self, data109: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str110(value110: int) -> TypeIs[str]:
    return False

def check_items110(items110: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha110: ...
class Beta110: ...

def check_alpha110(obj110: Alpha110) -> TypeIs[Beta110]:
    return False

class Validator110:
    def is_str_map(self, data110: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str111(value111: int) -> TypeIs[str]:
    return False

def check_items111(items111: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha111: ...
class Beta111: ...

def check_alpha111(obj111: Alpha111) -> TypeIs[Beta111]:
    return False

class Validator111:
    def is_str_map(self, data111: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str112(value112: int) -> TypeIs[str]:
    return False

def check_items112(items112: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha112: ...
class Beta112: ...

def check_alpha112(obj112: Alpha112) -> TypeIs[Beta112]:
    return False

class Validator112:
    def is_str_map(self, data112: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str113(value113: int) -> TypeIs[str]:
    return False

def check_items113(items113: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha113: ...
class Beta113: ...

def check_alpha113(obj113: Alpha113) -> TypeIs[Beta113]:
    return False

class Validator113:
    def is_str_map(self, data113: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str114(value114: int) -> TypeIs[str]:
    return False

def check_items114(items114: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha114: ...
class Beta114: ...

def check_alpha114(obj114: Alpha114) -> TypeIs[Beta114]:
    return False

class Validator114:
    def is_str_map(self, data114: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str115(value115: int) -> TypeIs[str]:
    return False

def check_items115(items115: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha115: ...
class Beta115: ...

def check_alpha115(obj115: Alpha115) -> TypeIs[Beta115]:
    return False

class Validator115:
    def is_str_map(self, data115: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str116(value116: int) -> TypeIs[str]:
    return False

def check_items116(items116: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha116: ...
class Beta116: ...

def check_alpha116(obj116: Alpha116) -> TypeIs[Beta116]:
    return False

class Validator116:
    def is_str_map(self, data116: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str117(value117: int) -> TypeIs[str]:
    return False

def check_items117(items117: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha117: ...
class Beta117: ...

def check_alpha117(obj117: Alpha117) -> TypeIs[Beta117]:
    return False

class Validator117:
    def is_str_map(self, data117: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str118(value118: int) -> TypeIs[str]:
    return False

def check_items118(items118: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha118: ...
class Beta118: ...

def check_alpha118(obj118: Alpha118) -> TypeIs[Beta118]:
    return False

class Validator118:
    def is_str_map(self, data118: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str119(value119: int) -> TypeIs[str]:
    return False

def check_items119(items119: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha119: ...
class Beta119: ...

def check_alpha119(obj119: Alpha119) -> TypeIs[Beta119]:
    return False

class Validator119:
    def is_str_map(self, data119: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str120(value120: int) -> TypeIs[str]:
    return False

def check_items120(items120: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha120: ...
class Beta120: ...

def check_alpha120(obj120: Alpha120) -> TypeIs[Beta120]:
    return False

class Validator120:
    def is_str_map(self, data120: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str121(value121: int) -> TypeIs[str]:
    return False

def check_items121(items121: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha121: ...
class Beta121: ...

def check_alpha121(obj121: Alpha121) -> TypeIs[Beta121]:
    return False

class Validator121:
    def is_str_map(self, data121: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str122(value122: int) -> TypeIs[str]:
    return False

def check_items122(items122: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha122: ...
class Beta122: ...

def check_alpha122(obj122: Alpha122) -> TypeIs[Beta122]:
    return False

class Validator122:
    def is_str_map(self, data122: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str123(value123: int) -> TypeIs[str]:
    return False

def check_items123(items123: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha123: ...
class Beta123: ...

def check_alpha123(obj123: Alpha123) -> TypeIs[Beta123]:
    return False

class Validator123:
    def is_str_map(self, data123: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str124(value124: int) -> TypeIs[str]:
    return False

def check_items124(items124: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha124: ...
class Beta124: ...

def check_alpha124(obj124: Alpha124) -> TypeIs[Beta124]:
    return False

class Validator124:
    def is_str_map(self, data124: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str125(value125: int) -> TypeIs[str]:
    return False

def check_items125(items125: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha125: ...
class Beta125: ...

def check_alpha125(obj125: Alpha125) -> TypeIs[Beta125]:
    return False

class Validator125:
    def is_str_map(self, data125: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str126(value126: int) -> TypeIs[str]:
    return False

def check_items126(items126: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha126: ...
class Beta126: ...

def check_alpha126(obj126: Alpha126) -> TypeIs[Beta126]:
    return False

class Validator126:
    def is_str_map(self, data126: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str127(value127: int) -> TypeIs[str]:
    return False

def check_items127(items127: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha127: ...
class Beta127: ...

def check_alpha127(obj127: Alpha127) -> TypeIs[Beta127]:
    return False

class Validator127:
    def is_str_map(self, data127: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str128(value128: int) -> TypeIs[str]:
    return False

def check_items128(items128: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha128: ...
class Beta128: ...

def check_alpha128(obj128: Alpha128) -> TypeIs[Beta128]:
    return False

class Validator128:
    def is_str_map(self, data128: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False

def check_str129(value129: int) -> TypeIs[str]:
    return False

def check_items129(items129: list[object]) -> TypeIs[list[int]]:
    return False

class Alpha129: ...
class Beta129: ...

def check_alpha129(obj129: Alpha129) -> TypeIs[Beta129]:
    return False

class Validator129:
    def is_str_map(self, data129: dict[str, int]) -> TypeIs[dict[str, str]]:
        return False
