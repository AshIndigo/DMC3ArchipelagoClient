struct VS_INPUT {
    float3 Pos : POSITION;
    float2 UV  : TEXCOORD0;
};

struct PS_INPUT {
    float4 Pos : SV_POSITION;
    float2 UV  : TEXCOORD0;
};

PS_INPUT main(VS_INPUT input) {
    PS_INPUT o;
    o.Pos = float4(input.Pos.xy, 0, 1); // screen space, no transform
    o.UV  = input.UV;
    return o;
}
