varying vec2 v_vt;
varying vec2 v_background_vt;

#ifdef VERTEX_SHADER
attribute vec2 a_pos;
uniform mat3 u_matrix;
uniform mat3 u_projection_matrix;
uniform mat3 u_view_matrix;
uniform vec2 u_background_pos;
uniform vec2 u_background_size;
void main() {
    v_vt = a_pos;
    vec3 world_pos = u_matrix * vec3(a_pos * 2.0 - 1.0, 1.0);
    v_background_vt = (world_pos.xy - u_background_pos) / u_background_size;
    vec3 pos = u_projection_matrix * u_view_matrix * world_pos;
    gl_Position = vec4(pos.xy, 0.0, pos.z);
}
#endif

#ifdef FRAGMENT_SHADER
uniform sampler2D u_texture;
uniform sampler2D u_furniture;
uniform vec4 u_color;
void main() {
    float a = texture2D(u_texture, v_vt).a;
    a *= texture2D(u_furniture, v_background_vt).a;
    gl_FragColor = vec4(u_color.xyz, a * 0.75);

}
#endif