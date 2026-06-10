import random
import requests
import json

# Configuración del servidor de Axum
BASE_URL = "http://127.0.1.1:3000"  # Cambia el puerto si usas otro

def generate_random_scene():
    materials = {}
    objects = []

    # 1. Suelo gigante (Gris muy claro)
    materials["ground"] = {
        "type": "lambertian",
        "albedo": [0.8, 0.8, 0.8]
    }
    objects.append({
        "type": "sphere",
        "center": [0.0, -1000.0, 0.0],
        "radius": 1000.0,
        "material": "ground"
    })

    # 2. Esferas grandes de control (Las tres joyas del centro del libro)
    # Esfera de vidrio perfecta
    materials["big_dielectric"] = {
        "type": "dielectric",
        "refraction_index": 1.5
    }
    objects.append({
        "type": "sphere",
        "center": [0.0, 1.0, 0.0],
        "radius": 1.0,
        "material": "big_dielectric"
    })

    # Esfera difusa café oscuro
    materials["big_lambertian"] = {
        "type": "lambertian",
        "albedo": [0.4, 0.2, 0.1]
    }
    objects.append({
        "type": "sphere",
        "center": [-4.0, 1.0, 0.0],
        "radius": 1.0,
        "material": "big_lambertian"
    })

    # Esfera metálica dorada pulida
    materials["big_metal"] = {
        "type": "metal",
        "albedo": [0.7, 0.6, 0.5],
        "fuzz": 0.0
    }
    objects.append({
        "type": "sphere",
        "center": [4.0, 1.0, 0.0],
        "radius": 1.0,
        "material": "big_metal"
    })

    # 3. Generación procedural de las ~484 esferas pequeñas (Grilla de -11 a 11)
    mat_counter = 0
    for a in range(-22, 22):
        for b in range(-22, 22):
            choose_mat = random.random()
            
            # Tamaño aleatorio sutil para darle dinamismo, promedio de 0.2
            radius = random.uniform(0.15, 0.25)
            
            # Posición con un pequeño desfase aleatorio para evitar que parezca un tablero rígido
            center = [
                a + 0.9 * random.random(),
                radius, # Apoyada exactamente sobre el suelo flotando según su radio
                b + 0.9 * random.random()
            ]

            # Evitar que las esferas pequeñas se metan dentro de las 3 esferas grandes principales
            from_big_1 = (center[0] - 0.0)**2 + (center[2] - 0.0)**2
            from_big_2 = (center[0] - (-4.0))**2 + (center[2] - 0.0)**2
            from_big_3 = (center[0] - 4.0)**2 + (center[2] - 0.0)**2
            
            if from_big_1 < 1.2 or from_big_2 < 1.2 or from_big_3 < 1.2:
                continue

            mat_name = f"sub_mat_{mat_counter}"
            mat_counter += 1

            if choose_mat < 0.4:
                # 🟢 Materiales Lambertian (Difusos aleatorios)
                albedo = [
                    random.random() * random.random(),
                    random.random() * random.random(),
                    random.random() * random.random()
                ]
                materials[mat_name] = {
                    "type": "lambertian",
                    "albedo": albedo
                }
                objects.append({
                    "type": "sphere",
                    "center": center,
                    "radius": radius,
                    "material": mat_name
                })

            elif choose_mat < 0.75:
                # 🟡 Materiales Metálicos (Albedo y rugosidad aleatoria)
                albedo = [
                    random.uniform(0.5, 1.0),
                    random.uniform(0.5, 1.0),
                    random.uniform(0.5, 1.0)
                ]
                fuzz = random.uniform(0.0, 0.5)
                materials[mat_name] = {
                    "type": "metal",
                    "albedo": albedo,
                    "fuzz": fuzz
                }
                objects.append({
                    "type": "sphere",
                    "center": center,
                    "radius": radius,
                    "material": mat_name
                })

            else:
                # 🔵 Materiales Dieléctricos (Vidrio / Agua / Vidrio hueco)
                is_hollow = random.random() > 0.5
                is_water = random.random() > 0.7
                
                ior = 1.333 if is_water else 1.5  # 1.333 Agua, 1.5 Vidrio corriente
                
                materials[mat_name] = {
                    "type": "dielectric",
                    "refraction_index": ior
                }
                
                # Añadir la esfera sólida de cristal
                objects.append({
                    "type": "sphere",
                    "center": center,
                    "radius": radius,
                    "material": mat_name
                })
                
                # Si se decide que es vidrio hueco, se mete una esfera invertida idéntica por dentro
                if is_hollow and not is_water:
                    mat_in_name = f"{mat_name}_hollow_in"
                    materials[mat_in_name] = {
                        "type": "dielectric",
                        "refraction_index": 1.0 / ior # Burbuja de aire interna
                    }
                    objects.append({
                        "type": "sphere",
                        "center": center,
                        "radius": radius * 0.85, # Ligeramente más chica
                        "material": mat_in_name
                    })

    return {"materials": materials, "objects": objects}


def main():
    print("🔮 Generando escena procedural aleatoria del libro...")
    scene_payload = generate_random_scene()
    
    # Parámetros de cámara idénticos al plano clásico del final de Shirley Book 1
    camera_payload = {
        "fov": 25.0,                  # FOV cerrado para dar efecto teleobjetivo dramático
        "look_from": [16.0, 2.0, 3.0], # Posición alta y lateral alejada
        "look_at": [0.0, 0.0, 0.0],    # Apuntando al centro de la escena
        "vup": [0.0, 1.0, 0.0],
        "samples_per_pixel": 1       # Sube a 500 para producción limpia sin grano
    }

    print(f"Total de objetos geométricos generados: {len(scene_payload['objects'])}")

    try:
        # 1. Actualizar la Cámara en el Servidor Rust
        print("\nSending 📷 CAMERA parameters to Axum...")
        cam_res = requests.put(f"{BASE_URL}/camera", json=camera_payload)
        if cam_res.status_code == 200:
            print(" -> Cámara configurada exitosamente.")
        else:
            print(f" -> Error en cámara: {cam_res.status_code}")

        # 2. Actualizar el Mundo (Scene)
        print("\nSending 🌍 WORLD geometry data to Axum...")
        scene_res = requests.post(f"{BASE_URL}/scene", json=scene_payload)
        if scene_res.status_code == 200:
            print(" -> Escena inyectada con éxito en el AppState.")
        else:
            print(f" -> Error en escena: {scene_res.status_code}")

        # 3. Disparar el Proceso de Renderizado
        print("\n🚀 Disparando señal de RENDER a la cola distribuidora...")
        render_res = requests.post(f"{BASE_URL}/render", json={}, headers={"Content-Type": "application/json"})
        if render_res.status_code == 200:
            print(" -> ¡Render iniciado! Los workers ya deben estar masticando píxeles.")
        else:
            print(f" -> Error al disparar render: {render_res.status_code}")
            print(render_res.text)

    except requests.exceptions.ConnectionError:
        print(f"\n❌ Error catastrófico: No se pudo conectar con el servidor web de Rust en {BASE_URL}.")
        print("Asegúrate de ejecutar cargo run dentro de rt-server primero.")

if __name__ == "__main__":
    main()
